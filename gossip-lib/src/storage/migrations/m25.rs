use crate::error::Error;
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::{EventV2, EventV3, TagV3};
use speedy::Readable;

impl Storage {
    pub(super) fn m25_trigger(&self) -> Result<(), Error> {
        self.db_events2()?;
        self.db_events3()?;
        Ok(())
    }

    pub(super) async fn m25_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: migrating events...");

        // Migrate
        self.m25_migrate_to_events3(txn).await?;

        Ok(())
    }

    async fn m25_migrate_to_events3<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let loop_txn = self.env.read_txn()?;
        let mut count: usize = 0;
        for result in self.db_events2()?.iter(&loop_txn)? {
            let (_key, val) = result?;
            let event2 = EventV2::read_from_buffer(val)?;
            let tags_json: String = serde_json::to_string(&event2.tags)?;
            let tags3: Vec<TagV3> = serde_json::from_str(&tags_json)?;
            let event3 = EventV3 {
                id: event2.id,
                pubkey: event2.pubkey,
                created_at: event2.created_at,
                kind: event2.kind,
                sig: event2.sig,
                content: event2.content,
                tags: tags3,
            };
            self.write_event3(&event3, Some(txn)).await?;
            count += 1;
        }

        tracing::info!("Migrated {} events", count);

        // clear events2 database (we don't have an interface to delete it)
        self.db_events2()?.clear(txn)?;

        Ok(())
    }
}
