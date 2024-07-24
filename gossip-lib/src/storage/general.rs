use crate::error::Error;
use crate::storage::{RawDatabase, Storage};
use heed::types::Bytes;
use std::sync::Mutex;

static GENERAL_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut GENERAL_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_general(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = GENERAL_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = GENERAL_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = GENERAL_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env().write_txn()?;
                let db = self
                    .env()
                    .database_options()
                    .types::<Bytes, Bytes>()
                    // no .flags needed
                    // unnamed!
                    .create(&mut txn)?;
                txn.commit()?;
                GENERAL_DB = Some(db);
                Ok(db)
            }
        }
    }
}
