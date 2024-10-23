use super::types::Handler;
use super::Table;
use crate::error::Error;
use crate::globals::GLOBALS;
use heed::types::Bytes;
use heed::Database;
use std::sync::Mutex;

static HANDLERS_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut HANDLERS_DB: Option<Database<Bytes, Bytes>> = None;

pub struct HandlersTable {}

impl Table for HandlersTable {
    type Item = Handler;

    fn lmdb_name() -> &'static str {
        "handlers"
    }

    fn db() -> Result<Database<Bytes, Bytes>, Error> {
        unsafe {
            if let Some(db) = HANDLERS_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = HANDLERS_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = HANDLERS_DB {
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
                HANDLERS_DB = Some(db);
                Ok(db)
            }
        }
    }
}
