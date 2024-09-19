use super::table::Table;
use super::{PersonTable, Storage};
use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Event, Filter, Id, Unixtime};
use std::collections::HashSet;

impl Storage {
    // Prune -------------------------------------------------------

    /// Remove all events (and related data) with a created_at before `from`
    /// and all related indexes.
    pub fn prune_old_events(&self, from: Unixtime) -> Result<usize, Error> {
        // Extract the Ids to delete.
        let txn = self.env.read_txn()?;
        let mut ids: HashSet<Id> = HashSet::new();
        for result in self.db_events()?.iter(&txn)? {
            let (_key, val) = result?;

            if let Some(created_at) = Event::get_created_at_from_speedy_bytes(val) {
                if created_at < from {
                    if let Some(id) = Event::get_id_from_speedy_bytes(val) {
                        // Do not prune bookmarks, regardless of how old they are
                        if GLOBALS.current_bookmarks.read().contains(&id) {
                            continue;
                        }

                        ids.insert(id);
                        // Too bad but we can't delete it now, other threads
                        // might try to access it still. We have to delete it from
                        // all the other maps first.
                    }
                }
            }
        }
        drop(txn);

        let mut txn = self.env.write_txn()?;

        // Delete from event_seen_on_relay
        let mut deletions: Vec<Vec<u8>> = Vec::new();
        for id in &ids {
            let start_key: &[u8] = id.as_slice();
            for result in self
                .db_event_seen_on_relay()?
                .prefix_iter(&txn, start_key)?
            {
                let (_key, val) = result?;
                deletions.push(val.to_owned());
            }
        }
        tracing::info!(
            "PRUNE: deleting {} records from event_seen_on_relay",
            deletions.len()
        );
        for deletion in deletions.drain(..) {
            self.db_event_seen_on_relay()?.delete(&mut txn, &deletion)?;
        }

        // Delete from event_viewed
        for id in &ids {
            let _ = self.db_event_viewed()?.delete(&mut txn, id.as_slice());
        }
        tracing::info!("PRUNE: deleted {} records from event_viewed", ids.len());

        // Delete from hashtags
        // (unfortunately since Ids are the values, we have to scan the whole thing)
        let mut deletions: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for result in self.db_hashtags()?.iter(&txn)? {
            let (key, val) = result?;
            let id = Id(val[0..32].try_into()?);
            if ids.contains(&id) {
                deletions.push((key.to_owned(), val.to_owned()));
            }
        }
        tracing::info!("PRUNE: deleting {} records from hashtags", deletions.len());
        for deletion in deletions.drain(..) {
            self.db_hashtags()?
                .delete_one_duplicate(&mut txn, &deletion.0, &deletion.1)?;
        }

        // Delete from relationships
        // (unfortunately because of the 2nd Id in the tag, we have to scan the whole thing)
        let mut deletions: Vec<Vec<u8>> = Vec::new();
        for result in self.db_relationships_by_id()?.iter(&txn)? {
            let (key, _val) = result?;
            let id = Id(key[0..32].try_into()?);
            if ids.contains(&id) {
                deletions.push(key.to_owned());
                continue;
            }
            let id2 = Id(key[32..64].try_into()?);
            if ids.contains(&id2) {
                deletions.push(key.to_owned());
            }
        }
        tracing::info!("PRUNE: deleting {} relationships", deletions.len());
        for deletion in deletions.drain(..) {
            self.db_relationships_by_id()?.delete(&mut txn, &deletion)?;
        }

        // delete from events
        for id in &ids {
            let _ = self.db_events()?.delete(&mut txn, id.as_slice());
        }
        tracing::info!("PRUNE: deleted {} records from events", ids.len());

        txn.commit()?;

        Ok(ids.len())
    }

    /// Prune people that are not used:
    ///   * No feed related events
    ///   * less than 6 events
    ///   * not in any lists
    ///   * not petnamed
    ///   * no valid nip05,
    ///
    /// Returns number of people deleted
    pub fn prune_unused_people<'a>(&'a self) -> Result<usize, Error> {
        let mut txn = self.get_write_txn()?;

        let ekinds = crate::enabled_event_kinds();
        let frkinds = crate::feed_related_event_kinds(true);

        let mut filter = Filter::new();
        filter.kinds = ekinds;
        filter.limit = Some(6);

        let mut count = 0;
        let loop_txn = self.env.read_txn()?;
        for person in PersonTable::iter(&loop_txn)? {
            // Keep if they are in a person list
            if !self.read_person_lists(&person.pubkey)?.is_empty() {
                continue;
            }

            // Keep if they have a petname
            if person.petname.is_some() {
                continue;
            }

            // Keep if they have a valid nip-05
            if person.nip05_valid {
                continue;
            }

            // Load up to 6 of their events
            filter.authors = vec![person.pubkey.into()];
            let events = match self.find_events_by_filter(&filter, |_| true) {
                Ok(events) => events,
                Err(_) => continue, // some error we can't handle right now
            };

            // Keep people with at least 6 events
            if events.len() >= 6 {
                continue;
            }

            // Keep if any of their events is feed related
            if events.iter().any(|e| frkinds.contains(&e.kind)) {
                continue;
            }

            count += 1;
            *GLOBALS.prune_status.write() = Some(
                person
                    .pubkey
                    .as_hex_string()
                    .get(0..10)
                    .unwrap_or("?")
                    .to_owned(),
            );

            // Delete their events
            for event in &events {
                self.delete_event(event.id, Some(&mut txn))?;
            }

            // Delete their person-relay records
            self.delete_person_relays(|pr| pr.pubkey == person.pubkey, Some(&mut txn))?;

            // Delete their person record
            PersonTable::delete_record(person.pubkey, Some(&mut txn))?;
        }

        tracing::info!("PRUNE: deleted {} records from people", count);

        txn.commit()?;

        Ok(count)
    }
}
