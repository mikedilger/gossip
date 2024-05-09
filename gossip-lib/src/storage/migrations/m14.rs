use heed::RwTxn;

use crate::error::Error;
use crate::storage::Storage;

impl Storage {
    pub(super) fn m14_trigger(&self) -> Result<(), Error> {
        Ok(())
    }

    pub(super) fn m14_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: removing a retired setting...");

        // Migrate
        self.m14_remove_setting_custom_person_list_names(txn)?;

        Ok(())
    }

    fn m14_remove_setting_custom_person_list_names<'a>(
        &'a self,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        self.general.delete(txn, b"custom_person_list_names")?;
        Ok(())
    }
}
