use super::Storage;
use crate::error::Error;
use heed::RwTxn;

impl Storage {
    pub(super) fn m42_trigger(&self) -> Result<(), Error> {
        Ok(())
    }

    pub(super) fn m42_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: rebuilding web of trust...");

        // Rebuild WoT
        self.set_flag_rebuild_wot_needed(true, Some(txn))?;

        Ok(())
    }
}
