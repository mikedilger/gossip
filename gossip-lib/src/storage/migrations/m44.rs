use crate::error::Error;
use crate::storage::Storage;
use heed::RwTxn;

impl Storage {
    pub(super) fn m44_trigger(&self) -> Result<(), Error> {
        Ok(())
    }

    pub(super) fn m44_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: Flagging that relationships need to be rebuilt...");

        // Rebuild relationships
        self.set_flag_rebuild_relationships_needed(true, Some(txn))?;

        Ok(())
    }
}
