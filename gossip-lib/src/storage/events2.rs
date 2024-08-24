use crate::error::Error;
use crate::storage::{RawDatabase, Storage};
use heed::types::Bytes;
use std::sync::Mutex;

// Id -> Event
//   key: id.as_slice() | Id(val[0..32].try_into()?)
//   val: event.write_to_vec() | Event::read_from_buffer(val)

static EVENTS2_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut EVENTS2_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_events2(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = EVENTS2_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = EVENTS2_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = EVENTS2_DB {
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
                    .name("events2")
                    .create(&mut txn)?;
                txn.commit()?;
                EVENTS2_DB = Some(db);
                Ok(db)
            }
        }
    }
}
