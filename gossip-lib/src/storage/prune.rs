use super::table::Table;
use super::{PersonTable, Storage};
use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Event, EventKind, EventReference, Filter, Id, PublicKey, Unixtime};
use speedy::Readable;
use std::collections::HashSet;

impl Storage {
    // Prune -------------------------------------------------------

    /// Remove all events (and related data) with a created_at before `from`
    /// and all related indexes.  Keep events from the user and all
    /// threads they have participated in, as well as bookmarks.
    pub fn prune_old_events(&self, from: Unixtime) -> Result<usize, Error> {

        // Extract the root IDs of threads that the user has participated in
        let mut roots: HashSet<EventReference> = HashSet::new();

        let user = GLOBALS.identity.public_key();
        if let Some(pk) = user {
            let mut filter = Filter::new();
            filter.add_author(pk);
            for event in self.find_events_by_filter(&filter, |_| true)? {
                if let Some(er) = event.replies_to_root() {
                    roots.insert(er);
                }
            }
            tracing::info!("Preserving {} conversations that you have participated in",
                           roots.len());
        }

        // Prepare
        let mut ids: HashSet<Id> = HashSet::new();
        let mut event_seen_on_relay_deletions: Vec<Vec<u8>> = Vec::new();
        let mut hashtag_deletions: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        let mut relationship_deletions: Vec<Vec<u8>> = Vec::new();
        {
            let txn = self.env.read_txn()?;

            // Extract the Ids of events to delete.
            for result in self.db_events()?.iter(&txn)? {
                let (_key, val) = result?;
                let event = Event::read_from_buffer(val)?;
                if event.created_at < from {

                    // Do not prune bookmarks, regardless of how old they are
                    if GLOBALS.current_bookmarks.read().contains(&event.id) {
                        continue;
                    }

                    // Do not prune certain kinds
                    // (this is probably incomplete)
                    if event.kind == EventKind::Metadata ||
                        event.kind == EventKind::ContactList ||
                        event.kind == EventKind::EncryptedDirectMessage ||
                        event.kind == EventKind::EventDeletion ||
                        event.kind == EventKind::GiftWrap ||
                        event.kind == EventKind::MuteList ||
                        event.kind == EventKind::PinList ||
                        event.kind == EventKind::RelayList ||
                        event.kind == EventKind::BookmarkList ||
                        event.kind == EventKind::FollowSets
                    {
                        continue;
                    }

                    if let Some(pk) = user {
                        // Do not prune any event authored by the user
                        if event.pubkey == pk {
                            continue;
                        }

                        // Do not prune any event that tags the user
                        if event.is_tagged(&pk) {
                            continue;
                        }

                        // Do not prune if part of a conversation that the user
                        // has engaged in
                        if let Some(er) = event.replies_to_root() {
                            if roots.contains(&er) {
                                continue;
                            }
                        }
                    }

                    ids.insert(event.id);
                    // Too bad but we can't delete it now, other threads
                    // might try to access it still. We have to delete it from
                    // all the other maps first.
                }
            }

            // Event seen on relay records
            for id in &ids {
                let start_key: &[u8] = id.as_slice();
                for result in self
                    .db_event_seen_on_relay()?
                    .prefix_iter(&txn, start_key)?
                {
                    let (_key, val) = result?;
                    event_seen_on_relay_deletions.push(val.to_owned());
                }
            }

            // Hashtag deletions
            // (unfortunately since Ids are the values, we have to scan the whole thing)
            for result in self.db_hashtags()?.iter(&txn)? {
                let (key, val) = result?;
                let id = Id(val[0..32].try_into()?);
                if ids.contains(&id) {
                    hashtag_deletions.push((key.to_owned(), val.to_owned()));
                }
            }

            // Relationship deletions
            // (unfortunately because of the 2nd Id in the tag, we have to scan the whole thing)
            for result in self.db_relationships_by_id()?.iter(&txn)? {
                let (key, _val) = result?;
                let id = Id(key[0..32].try_into()?);
                if ids.contains(&id) {
                    relationship_deletions.push(key.to_owned());
                    continue;
                }
                let id2 = Id(key[32..64].try_into()?);
                if ids.contains(&id2) {
                    relationship_deletions.push(key.to_owned());
                }
            }
        }

        // Actually delete
        {

            // Delete from event_seen_on_relay
            tracing::info!(
                "PRUNE: deleting {} records from event_seen_on_relay",
                event_seen_on_relay_deletions.len()
            );
            let mut txn = self.env.write_txn()?;
            for deletion in event_seen_on_relay_deletions.drain(..) {
                self.db_event_seen_on_relay()?.delete(&mut txn, &deletion)?;
            }
            txn.commit()?;

            // Delete from event_viewed
            let mut txn = self.env.write_txn()?;
            for (n, id) in ids.iter().enumerate() {
                self.db_event_viewed()?.delete(&mut txn, id.as_slice())?;
                if n % 100_000 == 0 {
                    txn.commit()?;
                    txn = self.env.write_txn()?;
                }
            }
            txn.commit()?;
            tracing::info!("PRUNE: deleted {} records from event_viewed", ids.len());


            // Delete from hashtags
            tracing::info!("PRUNE: deleting {} records from hashtags", hashtag_deletions.len());
            let mut txn = self.env.write_txn()?;
            for deletion in hashtag_deletions.drain(..) {
                self.db_hashtags()?
                    .delete_one_duplicate(&mut txn, &deletion.0, &deletion.1)?;
            }
            txn.commit()?;


            // Delete from relationships
            tracing::info!("PRUNE: deleting {} relationships", relationship_deletions.len());
            let mut txn = self.env.write_txn()?;
            for deletion in relationship_deletions.drain(..) {
                self.db_relationships_by_id()?.delete(&mut txn, &deletion)?;
            }
            txn.commit()?;

            // delete from events
            tracing::info!("PRUNE: deleting {} records from events", ids.len());
            let mut txn = self.env.write_txn()?;
            for (n, id) in ids.iter().enumerate() {
                self.db_events()?.delete(&mut txn, id.as_slice())?;
                if n % 100_000 == 0 {
                    txn.commit()?;
                    txn = self.env.write_txn()?;
                }
            }
            txn.commit()?;
            tracing::info!("PRUNE: complete");
        }

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
    pub fn prune_unused_people(&self) -> Result<usize, Error> {

        let ekinds = crate::enabled_event_kinds();
        let frkinds = crate::feed_related_event_kinds(true);

        let mut filter = Filter::new();
        filter.kinds = ekinds;
        filter.limit = Some(6);

        let mut count = 0;
        let loop_txn = self.env.read_txn()?;
        for (_pk, person) in PersonTable::iter(&loop_txn)? {
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
            filter.authors = vec![person.pubkey];
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

            tracing::info!("Deleting {}", person.pubkey.as_hex_string());

            let mut txn = self.get_write_txn()?;

            // Delete their events
            for event in &events {
                self.delete_event(event.id, Some(&mut txn))?;
            }

            // Delete their person-relay records
            self.delete_person_relays(|pr| pr.pubkey == person.pubkey, Some(&mut txn))?;

            // Delete their person record
            PersonTable::delete_record(person.pubkey, Some(&mut txn))?;

            txn.commit()?;
        }

        tracing::info!("PRUNE: deleted {} records from people", count);


        Ok(count)
    }

    /// Prune miscellaneous things
    pub fn prune_misc(&self) -> Result<(), Error> {
        let mut txn = self.get_write_txn()?;

        // Remove Fof entries with value=0
        let mut zero_fof: Vec<PublicKey> = Vec::new();
        {
            let iter = self.db_fof()?.iter(&txn)?;
            for result in iter {
                let (k, v) = result?;
                let pubkey = PublicKey::from_bytes(k, false)?;
                let count = u64::from_be_bytes(<[u8; 8]>::try_from(&v[..8]).unwrap());
                if count == 0 {
                    zero_fof.push(pubkey);
                }
            }
        }

        for pk in zero_fof.drain(..) {
            self.db_fof()?.delete(&mut txn, pk.as_bytes())?;
        }

        Ok(())
    }
}
