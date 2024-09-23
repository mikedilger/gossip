use super::types::Following;
use super::Table;
use crate::error::Error;
use crate::globals::GLOBALS;
use heed::types::Bytes;
use heed::Database;
use std::sync::Mutex;

static FOLLOWINGS_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut FOLLOWINGS_DB: Option<Database<Bytes, Bytes>> = None;

pub struct FollowingsTable {}

impl Table for FollowingsTable {
    type Item = Following;

    fn lmdb_name() -> &'static str {
        "followings"
    }

    fn db() -> Result<Database<Bytes, Bytes>, Error> {
        unsafe {
            if let Some(db) = FOLLOWINGS_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = FOLLOWINGS_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = FOLLOWINGS_DB {
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
                FOLLOWINGS_DB = Some(db);
                Ok(db)
            }
        }
    }
}
