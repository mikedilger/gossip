use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::storage::event_tag_index1::INDEXED_TAGS;
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::{EventKind, EventV1, PublicKeyHex};
use speedy::Readable;

impl Storage {
    pub(super) fn m11_trigger(&self) -> Result<(), Error> {
        let _ = self.db_events1()?;
        let _ = self.db_event_tag_index1()?;
        Ok(())
    }

    pub(super) fn m11_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: populating event tag index...");

        // Migrate
        self.m11_populate_event_tag_index(txn)?;

        Ok(())
    }

    fn m11_populate_event_tag_index<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let loop_txn = self.env.read_txn()?;
        for result in self.db_events1()?.iter(&loop_txn)? {
            let (_key, val) = result?;
            let event = EventV1::read_from_buffer(val)?;
            self.m11_write_event_tag_index1_event1(&event, txn)?;
        }

        Ok(())
    }

    // We had to copy this from event_tag_index1 which uses an unversioned Event
    pub fn m11_write_event_tag_index1_event1<'a>(
        &'a self,
        event: &EventV1,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        let mut event = event;

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

        // our user's public key
        let pk: Option<PublicKeyHex> = self.read_setting_public_key().map(|p| p.into());

        for tag in &event.tags {
            let tagname = tag.tagname();
            let value = match tag.value(1) {
                Ok(v) => v,
                Err(_) => continue, // no tag value, not indexable.
            };

            // Only index tags we intend to lookup later by tag.
            // If that set changes, (1) add to this code and (2) do a reindex migration
            if !INDEXED_TAGS.contains(&&*tagname) {
                continue;
            }
            // For 'p' tags, only index them if 'p' is our user
            if tagname == "p" {
                match &pk {
                    None => continue,
                    Some(pk) => {
                        if value != pk.as_str() {
                            continue;
                        }
                    }
                }
            }

            let mut key: Vec<u8> = tagname.as_bytes().to_owned();
            key.push(b'\"'); // double quote separator, unlikely to be inside of a tagname
            key.extend(value.as_bytes());
            let key = key!(&key); // limit the size
            let bytes = event.id.as_slice();
            self.db_event_tag_index1()?.put(txn, key, bytes)?;
        }

        Ok(())
    }
}
