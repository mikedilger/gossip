
use crate::error::Error;
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::EventV2;
use speedy::Readable;

impl Storage {
    pub(super) fn m17_trigger(&self) -> Result<(), Error> {
        let _ = self.db_relationships1();
        let _ = self.db_reprel1();
        Ok(())
    }

    pub(super) fn m17_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: ...");

        // Migrate
        self.m17_reindex_event_relationships(txn)?;

        Ok(())
    }

    fn m17_reindex_event_relationships<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        // Iterate through all events
        let loop_txn = self.env.read_txn()?;
        for result in self.db_events()?.iter(&loop_txn)? {
            let (_key, val) = result?;
            let event = EventV2::read_from_buffer(val)?;
            self.process_relationships_of_event(&event, Some(txn))?;
        }
        Ok(())
    }
}
