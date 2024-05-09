use heed::RwTxn;

use crate::error::Error;
use crate::storage::Storage;

impl Storage {
    pub(super) fn m18_trigger(&self) -> Result<(), Error> {
        let _ = self.db_relationships1();
        let _ = self.db_reprel1();
        let _ = self.db_relationships_by_id1();
        let _ = self.db_relationships_by_addr1();
        Ok(())
    }

    pub(super) fn m18_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: moving to new relationships storage...");

        // Migrate
        self.m18_move_to_new_relationships_storage(txn)?;

        Ok(())
    }

    fn m18_move_to_new_relationships_storage<'a>(
        &'a self,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Clear old relationships tables (we don't have an interface to delete it)
        self.db_relationships1()?.clear(txn)?;
        self.db_reprel1()?.clear(txn)?;

        self.set_flag_rebuild_relationships_needed(true, Some(txn))?;
        Ok(())
    }
}
