use crate::error::Error;
use crate::storage::types::RelationshipById1;
use crate::storage::{RawDatabase, Storage};
use heed::types::UnalignedSlice;
use heed::RwTxn;
use nostr_types::Id;
use speedy::{Readable, Writable};
use std::sync::Mutex;

// Id:Id -> RelationshipById1
//   key: id.as_slice(), id.as_slice() | Id(val[32..64].try_into()?)
//   val:  relationship_by_id.write_to_vec() | RelationshipById1::read_from_buffer(val)

// NOTE: this means the SECOND Id relates to the FIRST Id, e.g.
//     id2 replies to id1
//     id2 reacts to id1
//     id2 deletes id1
//     id2 is a zap receipt on id1

static RELATIONSHIPS_BY_ID1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut RELATIONSHIPS_BY_ID1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_relationships_by_id1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = RELATIONSHIPS_BY_ID1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = RELATIONSHIPS_BY_ID1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = RELATIONSHIPS_BY_ID1_DB {
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
                    .name("relationships_by_id1")
                    .create(&mut txn)?;
                txn.commit()?;
                RELATIONSHIPS_BY_ID1_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub(crate) fn write_relationship_by_id1<'a>(
        &'a self,
        id: Id,
        related: Id,
        relationship_by_id: RelationshipById1,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut key = id.as_ref().as_slice().to_owned();
        key.extend(related.as_ref());
        let value = relationship_by_id.write_to_vec()?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_relationships_by_id1()?.put(txn, &key, &value)?;
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

    pub(crate) fn find_relationships_by_id1(
        &self,
        id: Id,
    ) -> Result<Vec<(Id, RelationshipById1)>, Error> {
        let start_key = id.as_slice();
        let txn = self.env.read_txn()?;
        let iter = self
            .db_relationships_by_id1()?
            .prefix_iter(&txn, start_key)?;
        let mut output: Vec<(Id, RelationshipById1)> = Vec::new();
        for result in iter {
            let (key, val) = result?;
            let id2 = Id(key[32..64].try_into().unwrap());
            let relationship_by_id = RelationshipById1::read_from_buffer(val)?;
            output.push((id2, relationship_by_id));
        }
        Ok(output)
    }
}
