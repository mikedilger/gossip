use crate::error::Error;
use crate::storage::{RawDatabase, Storage};
use heed::types::UnalignedSlice;
use heed::RwTxn;
use nostr_types::{EventV2, Id};
use speedy::{Readable, Writable};
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
                    .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
                    // no .flags needed
                    .name("events2")
                    .create(&mut txn)?;
                txn.commit()?;
                EVENTS2_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub(crate) fn write_event2<'a>(
        &'a self,
        event: &EventV2,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        // write to lmdb 'events'
        let bytes = event.write_to_vec()?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_events2()?.put(txn, event.id.as_slice(), &bytes)?;

            // If giftwrap, index the inner rumor instead
            let mut eventptr: &EventV2 = event;
            let rumor: EventV2;
            if let Some(r) = self.switch_to_rumor(event, txn)? {
                rumor = r;
                eventptr = &rumor;
            }
            // also index the event
            self.write_event_ek_pk_index(eventptr.id, eventptr.kind, eventptr.pubkey, Some(txn))?;
            self.write_event_ek_c_index(
                eventptr.id,
                eventptr.kind,
                eventptr.created_at,
                Some(txn),
            )?;
            self.write_event_tag_index(eventptr, Some(txn))?;

            for hashtag in event.hashtags() {
                if hashtag.is_empty() {
                    continue;
                } // upstream bug
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

    pub(crate) fn read_event2(&self, id: Id) -> Result<Option<EventV2>, Error> {
        let txn = self.env.read_txn()?;
        match self.db_events2()?.get(&txn, id.as_slice())? {
            None => Ok(None),
            Some(bytes) => Ok(Some(EventV2::read_from_buffer(bytes)?)),
        }
    }

    pub(crate) fn has_event2(&self, id: Id) -> Result<bool, Error> {
        let txn = self.env.read_txn()?;
        match self.db_events2()?.get(&txn, id.as_slice())? {
            None => Ok(false),
            Some(_) => Ok(true),
        }
    }

    pub(crate) fn delete_event2<'a>(
        &'a self,
        id: Id,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let _ = self.db_events2()?.delete(txn, id.as_slice());
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
