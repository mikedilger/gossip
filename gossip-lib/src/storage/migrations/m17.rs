use crate::error::Error;
use crate::storage::Storage;
use heed::RwTxn;

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
        self.set_flag_rebuild_relationships_needed(true, Some(txn))?;
        Ok(())
    }
}
