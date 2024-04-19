use crate::error::Error;
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::{EventV1, EventV2, TagV2};
use speedy::Readable;

impl Storage {
    pub(super) fn m16_trigger(&self) -> Result<(), Error> {
        let _ = self.db_events1()?;
        let _ = self.db_events2()?;
        Ok(())
    }

    pub(super) fn m16_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: migrating events...");

        // Migrate
        self.m16_migrate_to_events2(txn)?;

        Ok(())
    }

    fn m16_migrate_to_events2<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let loop_txn = self.env.read_txn()?;
        let mut count: usize = 0;
        for result in self.db_events1()?.iter(&loop_txn)? {
            let (_key, val) = result?;
            let event1 = EventV1::read_from_buffer(val)?;
            let tags_json: String = serde_json::to_string(&event1.tags)?;
            let tags2: Vec<TagV2> = serde_json::from_str(&tags_json)?;
            let event2 = EventV2 {
                id: event1.id,
                pubkey: event1.pubkey,
                created_at: event1.created_at,
                kind: event1.kind,
                sig: event1.sig,
                content: event1.content,
                tags: tags2,
            };
            self.write_event2(&event2, Some(txn))?;
            count += 1;
        }

        tracing::info!("Migrated {} events", count);

        // clear events1 database (we don't have an interface to delete it)
        self.db_events1()?.clear(txn)?;

        Ok(())
    }
}
