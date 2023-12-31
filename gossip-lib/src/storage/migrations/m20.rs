use crate::error::Error;
use crate::globals::GLOBALS;
use crate::storage::types::PersonList1;
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::Tag;

impl Storage {
    pub(super) fn m20_trigger(&self) -> Result<(), Error> {
        let _ = self.db_person_lists_metadata1()?;
        Ok(())
    }

    pub(super) fn m20_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: initializing person list event metadata...");

        // Migrate
        self.m20_initialize_person_list_event_metadata(txn)?;

        Ok(())
    }

    fn m20_initialize_person_list_event_metadata<'a>(
        &'a self,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Get public key, or give up
        let pk = match self.read_setting_public_key() {
            Some(pk) => pk,
            None => return Ok(()),
        };

        for (list, mut metadata) in self.get_all_person_list_metadata1()? {
            if let Ok(Some(event)) =
                self.get_replaceable_event(list.event_kind(), pk, &metadata.dtag)
            {
                metadata.event_created_at = event.created_at;
                metadata.event_public_len = event
                    .tags
                    .iter()
                    .filter(|t| matches!(t, Tag::Pubkey { .. }))
                    .count();
                metadata.event_private_len = {
                    let mut private_len: Option<usize> = None;
                    if !matches!(list, PersonList1::Followed) && GLOBALS.identity.is_unlocked() {
                        if let Ok(bytes) = GLOBALS.identity.decrypt_nip04(&pk, &event.content) {
                            if let Ok(vectags) = serde_json::from_slice::<Vec<Tag>>(&bytes) {
                                private_len = Some(vectags.len());
                            }
                        }
                    }
                    private_len
                };
                self.set_person_list_metadata1(list, &metadata, Some(txn))?;
            }
        }

        Ok(())
    }
}
