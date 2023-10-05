use crate::error::Error;
use crate::relationship::Relationship;
use crate::storage::{RawDatabase, Storage};
use heed::types::UnalignedSlice;
use heed::RwTxn;
use nostr_types::Id;
use speedy::{Readable, Writable};
use std::sync::Mutex;

// Id:Id -> Relationship
//   key: id.as_slice(), id.as_slice() | Id(val[32..64].try_into()?)
//   val:  relationship.write_to_vec() | Relationship::read_from_buffer(val)

static RELATIONSHIPS1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut RELATIONSHIPS1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_relationships1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = RELATIONSHIPS1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = RELATIONSHIPS1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = RELATIONSHIPS1_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env.write_txn()?;
                let db = self
                    .env
                    .database_options()
                    .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
                    // no .flags needed?
                    .name("relationships")
                    .create(&mut txn)?;
                txn.commit()?;
                RELATIONSHIPS1_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub(crate) fn write_relationship1<'a>(
        &'a self,
        id: Id,
        related: Id,
        relationship: Relationship,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut key = id.as_ref().as_slice().to_owned();
        key.extend(related.as_ref());
        let value = relationship.write_to_vec()?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_relationships1()?.put(txn, &key, &value)?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub(crate) fn find_relationships1(&self, id: Id) -> Result<Vec<(Id, Relationship)>, Error> {
        let start_key = id.as_slice();
        let txn = self.env.read_txn()?;
        let iter = self.db_relationships1()?.prefix_iter(&txn, start_key)?;
        let mut output: Vec<(Id, Relationship)> = Vec::new();
        for result in iter {
            let (key, val) = result?;
            let id2 = Id(key[32..64].try_into().unwrap());
            let relationship = Relationship::read_from_buffer(val)?;
            output.push((id2, relationship));
        }
        Ok(output)
    }
}
