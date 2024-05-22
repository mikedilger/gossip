use super::types::Person3;
use super::Table;
use crate::error::Error;
use crate::globals::GLOBALS;
use heed::types::Bytes;
use heed::Database;
use std::sync::Mutex;

static PERSON3_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut PERSON3_DB: Option<Database<Bytes, Bytes>> = None;

pub struct Person3Table {}

impl Table for Person3Table {
    type Item = Person3;

    fn lmdb_name() -> &'static str {
        "person3"
    }

    fn db() -> Result<Database<Bytes, Bytes>, Error> {
        unsafe {
            if let Some(db) = PERSON3_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = PERSON3_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = PERSON3_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = GLOBALS.storage.env.write_txn()?;
                let db = GLOBALS
                    .storage
                    .env
                    .database_options()
                    .types::<Bytes, Bytes>()
                    .name(Self::lmdb_name())
                    .create(&mut txn)?;
                txn.commit()?;
                PERSON3_DB = Some(db);
                Ok(db)
            }
        }
    }

    fn newable() -> bool {
        true
    }
}
