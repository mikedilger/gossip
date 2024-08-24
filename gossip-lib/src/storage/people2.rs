use crate::error::Error;
use crate::storage::types::Person2;
use crate::storage::{RawDatabase, Storage};
use heed::types::Bytes;
use heed::RwTxn;
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
                    .types::<Bytes, Bytes>()
                    // no .flags needed
                    .name("people2")
                    .create(&mut txn)?;
                txn.commit()?;
                PEOPLE2_DB = Some(db);
                Ok(db)
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn write_person2<'a>(
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

        write_transact!(self, rw_txn, f)
    }
}
