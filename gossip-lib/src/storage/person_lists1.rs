use crate::error::Error;
use crate::people::PersonList;
use crate::storage::{RawDatabase, Storage};
use heed::types::UnalignedSlice;
use heed::RwTxn;
use nostr_types::PublicKey;
use std::sync::Mutex;

// Pubkey -> Vec<u8>
//   key: pubkey.as_bytes()

static PERSON_LISTS1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut PERSON_LISTS1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_person_lists1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = PERSON_LISTS1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = PERSON_LISTS1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = PERSON_LISTS1_DB {
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
                    .name("person_lists")
                    .create(&mut txn)?;
                txn.commit()?;
                PERSON_LISTS1_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub(crate) fn write_person_lists1<'a>(
        &'a self,
        pubkey: &PublicKey,
        lists: Vec<PersonList>,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let key: Vec<u8> = pubkey.to_bytes();
        let bytes = lists.iter().map(|l| (*l).into()).collect::<Vec<u8>>();

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_person_lists1()?.put(txn, &key, &bytes)?;
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
}
