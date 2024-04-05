use crate::error::Error;
use crate::storage::Storage;
use heed::RwTxn;

impl Storage {
    pub(super) fn m30_trigger(&self) -> Result<(), Error> {
        let _ = self.db_event_ek_pk_index1()?;
        let _ = self.db_event_ek_c_index1()?;
        Ok(())
    }

    pub(super) fn m30_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: Clearing old indexes...");

        // Migrate
        self.m30_clear_old_indexes(txn)?;

        Ok(())
    }

    fn m30_clear_old_indexes<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        self.db_event_ek_pk_index1()?.clear(txn)?;
        self.db_event_ek_c_index1()?.clear(txn)?;
        Ok(())
    }
}
