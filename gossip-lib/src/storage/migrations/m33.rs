use crate::error::Error;
use crate::storage::Storage;
use heed::RwTxn;

impl Storage {
    pub(super) fn m33_trigger(&self) -> Result<(), Error> {
        Ok(())
    }

    pub(super) fn m33_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: clearing indexes, flagging need to rebuild...");

        // Migrate
        self.m33_rebuild_indexes(txn)?;

        Ok(())
    }

    fn m33_rebuild_indexes<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        self.set_flag_rebuild_indexes_needed(true, Some(txn))?;

        Ok(())
    }
}
