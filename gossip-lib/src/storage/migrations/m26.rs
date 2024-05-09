use heed::RwTxn;

use crate::error::Error;
use crate::storage::Storage;

impl Storage {
    pub(super) fn m26_trigger(&self) -> Result<(), Error> {
        Ok(())
    }

    pub(super) fn m26_migrate<'a>(
        &'a self,
        prefix: &str,
        _txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: migration is now null.");

        Ok(())
    }
}
