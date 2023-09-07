use crate::error::Error;
use crate::storage::types::Person2;
use crate::storage::{RawDatabase, Storage};
use heed::types::UnalignedSlice;
use heed::RwTxn;
use nostr_types::PublicKey;
use std::sync::Mutex;

// PublicKey -> Person
//   key: pubkey.as_bytes()
//   val: serde_json::to_vec(person) | serde_json::from_slice(bytes)

static PEOPLE2_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut PEOPLE2_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_people2(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = PEOPLE2_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = PEOPLE2_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = PEOPLE2_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env.write_txn()?;
                let db = self
                    .env
                    .database_options()
                    .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
                    .name("people")
                    .create(&mut txn)?;
                txn.commit()?;
                PEOPLE2_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub fn get_people2_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.db_people2()?.len(&txn)?)
    }

    #[allow(dead_code)]
    pub fn write_person2<'a>(
        &'a self,
        person: &Person2,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key: Vec<u8> = person.pubkey.to_bytes();
        let bytes = serde_json::to_vec(person)?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_people2()?.put(txn, &key, &bytes)?;
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

    pub fn read_person2(&self, pubkey: &PublicKey) -> Result<Option<Person2>, Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key: Vec<u8> = pubkey.to_bytes();
        let txn = self.env.read_txn()?;
        Ok(match self.db_people2()?.get(&txn, &key)? {
            Some(bytes) => Some(serde_json::from_slice(bytes)?),
            None => None,
        })
    }

    pub fn filter_people2<F>(&self, f: F) -> Result<Vec<Person2>, Error>
    where
        F: Fn(&Person2) -> bool,
    {
        let txn = self.env.read_txn()?;
        let iter = self.db_people2()?.iter(&txn)?;
        let mut output: Vec<Person2> = Vec::new();
        for result in iter {
            let (_key, val) = result?;
            let person: Person2 = serde_json::from_slice(val)?;
            if f(&person) {
                output.push(person);
            }
        }
        Ok(output)
    }
}
