use crate::error::Error;
use crate::relationship::Relationship;
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::{EventReference, EventV1};
use speedy::Readable;

impl Storage {
    pub(super) fn m1_trigger(&self) -> Result<(), Error> {
        let _ = self.db_events1()?;
        Ok(())
    }

    pub(super) fn m1_migrate<'a>(&'a self, prefix: &str, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let read_txn = self.env.read_txn()?;
        let total = self.db_events1()?.len(&read_txn)?;

        // Info message
        tracing::info!("{prefix}: Computing and storing event relationships for {total} events...");

        // Migrate
        let mut count = 0;
        let event_txn = self.env.read_txn()?;
        for result in self.db_events1()?.iter(&event_txn)? {
            let pair = result?;
            let event = EventV1::read_from_buffer(pair.1)?;
            self.m1_process_relationships_of_event(&event, txn)?;
            count += 1;
            for checkpoint in &[10, 20, 30, 40, 50, 60, 70, 80, 90] {
                if count == checkpoint * total / 100 {
                    tracing::info!("{}% done", checkpoint);
                }
            }
        }

        tracing::info!("syncing...");

        Ok(())
    }

    /// Process relationships of an eventv1.
    fn m1_process_relationships_of_event<'a>(
        &'a self,
        event: &EventV1,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // replies to
        match event.replies_to() {
            Some(EventReference::Id(id, _, _)) => {
                self.write_relationship1(id, event.id, Relationship::Reply, Some(txn))?;
            }
            Some(EventReference::Addr(_ea)) => {
                // will only work if we already have it... yuck.
                // We need a new relationships database for EventAddrs
                // FIXME
            }
            None => (),
        }

        // reacts to
        if let Some((reacted_to_id, reaction, _maybe_url)) = event.reacts_to() {
            if let Some(reacted_to_event) = self.read_event1(reacted_to_id)? {
                // Only if they are different people (no liking your own posts)
                if reacted_to_event.pubkey != event.pubkey {
                    self.write_relationship1(
                        reacted_to_id, // event reacted to
                        event.id,      // the reaction event id
                        Relationship::Reaction(event.pubkey, reaction),
                        Some(txn),
                    )?;
                }
            } else {
                // Store the reaction to the event we dont have yet.
                // We filter bad ones when reading them back too, so even if this
                // turns out to be a reaction by the author, they can't like
                // their own post
                self.write_relationship1(
                    reacted_to_id, // event reacted to
                    event.id,      // the reaction event id
                    Relationship::Reaction(event.pubkey, reaction),
                    Some(txn),
                )?;
            }
        }

        // deletes
        if let Some((deleted_event_ids, reason)) = event.deletes() {
            for deleted_event_id in deleted_event_ids {
                // since it is a delete, we don't actually desire the event.
                if let Some(deleted_event) = self.read_event1(deleted_event_id)? {
                    // Only if it is the same author
                    if deleted_event.pubkey == event.pubkey {
                        self.write_relationship1(
                            deleted_event_id,
                            event.id,
                            Relationship::Deletion(reason.clone()),
                            Some(txn),
                        )?;
                    }
                } else {
                    // We don't have the deleted event. Presume it is okay. We check again
                    // when we read these back
                    self.write_relationship1(
                        deleted_event_id,
                        event.id,
                        Relationship::Deletion(reason.clone()),
                        Some(txn),
                    )?;
                }
            }
        }

        // zaps
        match event.zaps() {
            Ok(Some(zapdata)) => {
                self.write_relationship1(
                    zapdata.id,
                    event.id,
                    Relationship::ZapReceipt(event.pubkey, zapdata.amount),
                    Some(txn),
                )?;
            }
            Err(e) => tracing::error!("Invalid zap receipt: {}", e),
            _ => {}
        }

        Ok(())
    }
}
