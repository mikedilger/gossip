use crate::error::Error;
use crate::storage::{RawDatabase, Storage};
use heed::{types::Bytes, DatabaseFlags};
use std::sync::Mutex;

// Kind:Pubkey:d-tag -> Relationship1:Id
//   (has dups)

static REPREL1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut REPREL1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_reprel1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = REPREL1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = REPREL1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = REPREL1_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env.write_txn()?;
                let db = self
                    .env
                    .database_options()
                    .types::<Bytes, Bytes>()
                    .flags(DatabaseFlags::DUP_SORT) // NOT FIXED, Relationship1 serialized isn't.
                    .name("reprel1")
                    .create(&mut txn)?;
                txn.commit()?;
                REPREL1_DB = Some(db);
                Ok(db)
            }
        }
    }
}
