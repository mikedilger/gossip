use heed::types::UnalignedSlice;
use heed::{DatabaseFlags, RwTxn};

use crate::error::Error;
use crate::storage::Storage;

impl Storage {
    pub(super) fn m12_trigger(&self) -> Result<(), Error> {
        Ok(())
    }

    pub(super) fn m12_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: removing now unused event_references_person index...");

        // Migrate
        self.m12_remove_event_references_person(txn)?;

        Ok(())
    }

    fn m12_remove_event_references_person<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        {
            let db = self
                .env
                .database_options()
                .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
                .flags(DatabaseFlags::DUP_SORT | DatabaseFlags::DUP_FIXED)
                .name("event_references_person")
                .create(txn)?;

            db.clear(txn)?;
        }

        // heed doesn't expose mdb_drop(1) yet, so we can't actually remove this
        // database.

        Ok(())
    }
}
