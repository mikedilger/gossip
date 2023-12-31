use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::{EventKind, EventV1};
use speedy::Readable;

impl Storage {
    pub(super) fn m9_trigger(&self) -> Result<(), Error> {
        let _ = self.db_events1()?;
        let _ = self.db_event_ek_pk_index1()?;
        let _ = self.db_event_ek_c_index1()?;
        let _ = self.db_hashtags1()?;
        Ok(())
    }

    pub(super) fn m9_migrate<'a>(&'a self, prefix: &str, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: rebuilding event indices...");

        // Migrate
        self.m9_rebuild_event_indices(txn)?;

        Ok(())
    }

    /// Rebuild all the event indices. This is generally internal, but might be used
    /// to fix a broken database.
    pub fn m9_rebuild_event_indices<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        // Erase all indices first
        self.db_event_ek_pk_index1()?.clear(txn)?;
        self.db_event_ek_c_index1()?.clear(txn)?;
        self.db_hashtags1()?.clear(txn)?;

        let loop_txn = self.env.read_txn()?;
        for result in self.db_events1()?.iter(&loop_txn)? {
            let (_key, val) = result?;
            let event = EventV1::read_from_buffer(val)?;
            self.m9_write_event_indices(&event, txn)?;
            for hashtag in event.hashtags() {
                if hashtag.is_empty() {
                    continue;
                } // upstream bug
                self.add_hashtag1(&hashtag, event.id, Some(txn))?;
            }
        }

        Ok(())
    }

    fn m9_write_event_indices<'a>(
        &'a self,
        event: &EventV1,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        let mut event = event;

        // If giftwrap, index the inner rumor instead
        let mut rumor_event: EventV1;
        if event.kind == EventKind::GiftWrap {
            match GLOBALS.identity.unwrap_giftwrap1(event) {
                Ok(rumor) => {
                    rumor_event = rumor.into_event_with_bad_signature();
                    rumor_event.id = event.id; // lie, so it indexes it under the giftwrap
                    event = &rumor_event;
                }
                Err(e) => {
                    if matches!(e.kind, ErrorKind::NoPrivateKey) {
                        // Store as unindexed for later indexing
                        let bytes = vec![];
                        self.db_unindexed_giftwraps1()?
                            .put(txn, event.id.as_slice(), &bytes)?;
                    }
                }
            }
        }

        let ek: u32 = event.kind.into();

        let mut key: Vec<u8> = ek.to_be_bytes().as_slice().to_owned(); // event kind
        key.extend(event.pubkey.as_bytes()); // pubkey
        let bytes = event.id.as_slice();
        self.db_event_ek_pk_index()?.put(txn, &key, bytes)?;

        let mut key: Vec<u8> = ek.to_be_bytes().as_slice().to_owned(); // event kind
        key.extend((i64::MAX - event.created_at.0).to_be_bytes().as_slice()); // reverse created_at
        let bytes = event.id.as_slice();
        self.db_event_ek_c_index()?.put(txn, &key, bytes)?;

        Ok(())
    }
}
