use super::Storage;
use crate::error::Error;
use nostr_types::{Event, Id, Unixtime};
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
}
