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
        tracing::info!("{prefix}: rebuilding friends-of-friends datat...");

        // Rebuild Fof
        self.set_flag_rebuild_fof_needed(true, Some(txn))?;

        Ok(())
    }
}
