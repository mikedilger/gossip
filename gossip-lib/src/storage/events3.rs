use crate::error::Error;
use crate::storage::{RawDatabase, Storage};
use heed::types::UnalignedSlice;
use heed::RwTxn;
use nostr_types::{EventV3, Id};
use speedy::{Readable, Writable};
use std::sync::Mutex;

// Id -> Event
//   key: id.as_slice() | Id(val[0..32].try_into()?)
//   val: event.write_to_vec() | Event::read_from_buffer(val)

static EVENTS3_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut EVENTS3_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_events3(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = EVENTS3_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = EVENTS3_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = EVENTS3_DB {
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
                    .name("events3")
                    .create(&mut txn)?;
                txn.commit()?;
                EVENTS3_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub(crate) fn write_event3<'a>(
        &'a self,
        event: &EventV3,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        // write to lmdb 'events'
        let bytes = event.write_to_vec()?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_events3()?.put(txn, event.id.as_slice(), &bytes)?;

            // If giftwrap:
            //   Use the id and kind of the giftwrap,
            //   Use the pubkey and created_at of the rumor
            let mut innerevent: &EventV3 = event;
            let rumor: EventV3;
            if let Some(r) = self.switch_to_rumor3(event, txn)? {
                rumor = r;
                innerevent = &rumor;
            }

            // also index the event
            self.write_event_akci_index(
                innerevent.pubkey,
                event.kind,
                innerevent.created_at,
                event.id,
                Some(txn),
            )?;
            self.write_event_kci_index(event.kind, innerevent.created_at, event.id, Some(txn))?;
            self.write_event3_tag_index1(
                &event, // use the outer giftwrap event
                Some(txn),
            )?;

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

    pub(crate) fn read_event3(&self, id: Id) -> Result<Option<EventV3>, Error> {
        let txn = self.env.read_txn()?;
        match self.db_events3()?.get(&txn, id.as_slice())? {
            None => Ok(None),
            Some(bytes) => Ok(Some(EventV3::read_from_buffer(bytes)?)),
        }
    }

    pub(crate) fn has_event3(&self, id: Id) -> Result<bool, Error> {
        let txn = self.env.read_txn()?;
        match self.db_events3()?.get(&txn, id.as_slice())? {
            None => Ok(false),
            Some(_) => Ok(true),
        }
    }

    pub(crate) fn delete_event3<'a>(
        &'a self,
        id: Id,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let _ = self.db_events3()?.delete(txn, id.as_slice());
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
