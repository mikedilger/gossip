use crate::error::Error;
use crate::globals::GLOBALS;
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::{EventKind, EventV3};

impl Storage {
    pub(super) async fn switch_to_rumor3<'a>(
        &'a self,
        event: &EventV3,
        txn: &mut RwTxn<'a>,
    ) -> Result<Option<EventV3>, Error> {
        if event.kind == EventKind::GiftWrap {
            match GLOBALS.identity.unwrap_giftwrap(event).await {
                Ok(rumor) => {
                    let mut rumor_event = rumor.into_event_with_bad_signature();
                    rumor_event.id = event.id; // lie, so it indexes it under the giftwrap
                    Ok(Some(rumor_event))
                }
                Err(_) => {
                    // Store as unindexed for later indexing
                    let bytes = vec![];
                    self.db_unindexed_giftwraps()?
                        .put(txn, event.id.as_slice(), &bytes)?;
                    // and do not throw the error
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }
}
