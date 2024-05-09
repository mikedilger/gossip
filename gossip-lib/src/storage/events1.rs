use std::sync::Mutex;

use heed::types::UnalignedSlice;
use nostr_types::{EventV1, Id};
use speedy::Readable;

use crate::error::Error;
use crate::storage::{RawDatabase, Storage};

// Id -> Event
//   key: id.as_slice() | Id(val[0..32].try_into()?)
//   val: event.write_to_vec() | Event::read_from_buffer(val)

static EVENTS1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut EVENTS1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_events1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = EVENTS1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = EVENTS1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = EVENTS1_DB {
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
                    .name("events")
                    .create(&mut txn)?;
                txn.commit()?;
                EVENTS1_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub(crate) fn read_event1(&self, id: Id) -> Result<Option<EventV1>, Error> {
        let txn = self.env.read_txn()?;
        match self.db_events1()?.get(&txn, id.as_slice())? {
            None => Ok(None),
            Some(bytes) => Ok(Some(EventV1::read_from_buffer(bytes)?)),
        }
    }
}
