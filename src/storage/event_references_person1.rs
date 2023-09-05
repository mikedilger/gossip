use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::storage::{RawDatabase, Storage};
use heed::types::UnalignedSlice;
use heed::RwTxn;
use nostr_types::{Event, EventKind, Id, PublicKey, Unixtime};
use speedy::Readable;
use std::collections::HashSet;
use std::ops::Bound;
use std::sync::Mutex;

// PublicKey:ReverseUnixtime -> Id
// (pubkey is referenced by the event somehow)
// (only feed-displayable events are included)
// (dup keys, so multiple Ids per key)
// NOTE: this may be far too much data. Maybe we should only build this for the
//       user's pubkey as their inbox.

static EVENT_REFERENCES_PERSON1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut EVENT_REFERENCES_PERSON1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_event_references_person1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = EVENT_REFERENCES_PERSON1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = EVENT_REFERENCES_PERSON1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = EVENT_REFERENCES_PERSON1_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env.write_txn()?;
                let db = self
                    .env
                    .database_options()
                    .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
                    .name("event_references_person")
                    .create(&mut txn)?;
                txn.commit()?;
                EVENT_REFERENCES_PERSON1_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub fn write_event_references_person1<'a>(
        &'a self,
        event: &Event,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let mut event = event;

            // If giftwrap, index the inner rumor instead
            let mut rumor_event: Event;
            if event.kind == EventKind::GiftWrap {
                match GLOBALS.signer.unwrap_giftwrap(event) {
                    Ok(rumor) => {
                        rumor_event = rumor.into_event_with_bad_signature();
                        rumor_event.id = event.id; // lie, so it indexes it under the giftwrap
                        event = &rumor_event;
                    }
                    Err(e) => {
                        if matches!(e.kind, ErrorKind::NoPrivateKey) {
                            // Store as unindexed for later indexing
                            let bytes = vec![];
                            self.db_unindexed_giftwraps()?
                                .put(txn, event.id.as_slice(), &bytes)?;
                        }
                    }
                }
            }

            if !event.kind.is_feed_displayable() {
                return Ok(());
            }

            let bytes = event.id.as_slice();

            let mut pubkeys: HashSet<PublicKey> = HashSet::new();
            for (pubkeyhex, _, _) in event.people() {
                let pubkey = match PublicKey::try_from_hex_string(pubkeyhex.as_str(), false) {
                    Ok(pk) => pk,
                    Err(_) => continue,
                };
                pubkeys.insert(pubkey);
            }
            for pubkey in event.people_referenced_in_content() {
                pubkeys.insert(pubkey);
            }
            if !pubkeys.is_empty() {
                for pubkey in pubkeys.drain() {
                    let mut key: Vec<u8> = pubkey.to_bytes();
                    key.extend((i64::MAX - event.created_at.0).to_be_bytes().as_slice()); // reverse created_at
                    self.db_event_references_person1()?.put(txn, &key, bytes)?;
                }
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

    // Read all events referencing a given person in reverse time order
    pub fn read_events_referencing_person1<F>(
        &self,
        pubkey: &PublicKey,
        since: Unixtime,
        f: F,
    ) -> Result<Vec<Event>, Error>
    where
        F: Fn(&Event) -> bool,
    {
        let txn = self.env.read_txn()?;
        let now = Unixtime::now().unwrap();
        let mut start_key: Vec<u8> = pubkey.to_bytes();
        let mut end_key: Vec<u8> = start_key.clone();
        start_key.extend((i64::MAX - now.0).to_be_bytes().as_slice()); // work back from now
        end_key.extend((i64::MAX - since.0).to_be_bytes().as_slice()); // until since
        let range = (Bound::Included(&*start_key), Bound::Excluded(&*end_key));
        let iter = self.db_event_references_person1()?.range(&txn, &range)?;
        let mut events: Vec<Event> = Vec::new();
        for result in iter {
            let (_key, val) = result?;

            // Take the event
            let id = Id(val[0..32].try_into()?);
            // (like read_event, but we supply our on transaction)
            if let Some(bytes) = self.db_events1()?.get(&txn, id.as_slice())? {
                let event = Event::read_from_buffer(bytes)?;
                if f(&event) {
                    events.push(event);
                }
            }
        }
        Ok(events)
    }
}
