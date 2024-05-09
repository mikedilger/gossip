use std::sync::Mutex;

use heed::types::UnalignedSlice;
use heed::RwTxn;
use speedy::{Readable, Writable};

use super::types::{PersonList1, PersonListMetadata2};
use crate::error::Error;
use crate::storage::{RawDatabase, Storage};

// PersonList1 -> PersonListMetadata2 // bool is if private or not

static PERSON_LISTS_METADATA2_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut PERSON_LISTS_METADATA2_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_person_lists_metadata2(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = PERSON_LISTS_METADATA2_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = PERSON_LISTS_METADATA2_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = PERSON_LISTS_METADATA2_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env.write_txn()?;
                let db = self
                    .env
                    .database_options()
                    .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
                    // no .flags needed
                    .name("person_lists_metadata2")
                    .create(&mut txn)?;
                txn.commit()?;
                PERSON_LISTS_METADATA2_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub(crate) fn set_person_list_metadata2<'a>(
        &'a self,
        list: PersonList1,
        metadata: &PersonListMetadata2,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let key: Vec<u8> = list.write_to_vec()?;

        // Do not allow overwriting dtag or title of well defined lists:
        let bytes: Vec<u8> = if list == PersonList1::Muted {
            let mut md = metadata.to_owned();
            md.dtag = "muted".to_owned();
            md.title = "Muted".to_owned();
            md.write_to_vec()?
        } else if list == PersonList1::Followed {
            let mut md = metadata.to_owned();
            md.dtag = "followed".to_owned();
            md.title = "Followed".to_owned();
            md.write_to_vec()?
        } else {
            metadata.write_to_vec()?
        };

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_person_lists_metadata2()?.put(txn, &key, &bytes)?;
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

    pub(crate) fn get_all_person_list_metadata2(
        &self,
    ) -> Result<Vec<(PersonList1, PersonListMetadata2)>, Error> {
        let txn = self.env.read_txn()?;
        let mut output: Vec<(PersonList1, PersonListMetadata2)> = Vec::new();
        for result in self.db_person_lists_metadata2()?.iter(&txn)? {
            let (key, val) = result?;
            let list = PersonList1::read_from_buffer(key)?;
            let metadata = PersonListMetadata2::read_from_buffer(val)?;
            output.push((list, metadata));
        }
        Ok(output)
    }
}
