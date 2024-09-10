use super::types::Person4;
use super::Table;
use crate::error::Error;
use crate::globals::GLOBALS;
use heed::types::Bytes;
use heed::Database;
use std::sync::Mutex;

static PERSON4_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut PERSON4_DB: Option<Database<Bytes, Bytes>> = None;

pub struct Person4Table {}

impl Table for Person4Table {
    type Item = Person4;

    fn lmdb_name() -> &'static str {
        "person4"
    }

    fn db() -> Result<Database<Bytes, Bytes>, Error> {
        unsafe {
            if let Some(db) = PERSON4_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = PERSON4_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = PERSON4_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = GLOBALS.db().env.write_txn()?;
                let db = GLOBALS
                    .db()
                    .env
                    .database_options()
                    .types::<Bytes, Bytes>()
                    .name(Self::lmdb_name())
                    .create(&mut txn)?;
                txn.commit()?;
                PERSON4_DB = Some(db);
                Ok(db)
            }
        }
    }
}
