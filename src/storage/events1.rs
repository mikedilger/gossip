use crate::error::Error;
use crate::storage::{RawDatabase, Storage};
use heed::types::UnalignedSlice;
use heed::RwTxn;
use nostr_types::{Event, Id};
use speedy::{Readable, Writable};
use std::sync::Mutex;

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

    pub fn write_event1<'a>(
        &'a self,
        event: &Event,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        // write to lmdb 'events'
        let bytes = event.write_to_vec()?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_events1()?.put(txn, event.id.as_slice(), &bytes)?;

            // also index the event
            self.write_event_ek_pk_index(event, Some(txn))?;
            self.write_event_ek_c_index(event, Some(txn))?;
            self.write_event_references_person(event, Some(txn))?;
            for hashtag in event.hashtags() {
                if hashtag.is_empty() { continue; } // upstream bug
                self.add_hashtag(&hashtag, event.id, Some(txn))?;
            }
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

    pub fn read_event1(&self, id: Id) -> Result<Option<Event>, Error> {
        let txn = self.env.read_txn()?;
        match self.db_events1()?.get(&txn, id.as_slice())? {
            None => Ok(None),
            Some(bytes) => Ok(Some(Event::read_from_buffer(bytes)?)),
        }
    }

    pub fn has_event1(&self, id: Id) -> Result<bool, Error> {
        let txn = self.env.read_txn()?;
        match self.db_events1()?.get(&txn, id.as_slice())? {
            None => Ok(false),
            Some(_) => Ok(true),
        }
    }

    pub fn delete_event1<'a>(
        &'a self,
        id: Id,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let _ = self.db_events1()?.delete(txn, id.as_slice());
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
}
