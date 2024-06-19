use crate::error::Error;
use crate::storage::Storage;
use heed::RwTxn;

impl Storage {
    pub(super) fn m39_trigger(&self) -> Result<(), Error> {
        Ok(())
    }

    pub(super) fn m39_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: Flagging that relay lists need reprocessing...");

        // Migrate
        self.m39_flag_reprocess_relay_lists(txn)?;

        Ok(())
    }

    fn m39_flag_reprocess_relay_lists<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        self.set_flag_reprocess_relay_lists_needed(true, Some(txn))?;
        Ok(())
    }
}
