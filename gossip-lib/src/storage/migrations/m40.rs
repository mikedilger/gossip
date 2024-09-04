use crate::error::Error;
use crate::storage::Storage;
use heed::RwTxn;

impl Storage {
    pub(super) fn m40_trigger(&self) -> Result<(), Error> {
        let _ = self.db_relationships_by_addr2()?;
        let _ = self.db_relationships_by_addr3()?;
        Ok(())
    }

    pub(super) fn m40_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: Flagging that relationships need to be rebuilt...");

        // Migrate
        self.m40_migrate_relationship_data(txn)?;

        Ok(())
    }

    fn m40_migrate_relationship_data<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        // Clear the old relationships data
        self.db_relationships_by_addr2()?.clear(txn)?;

        // Rebuild relationships
        self.set_flag_rebuild_relationships_needed(true, Some(txn))?;

        Ok(())
    }
}
