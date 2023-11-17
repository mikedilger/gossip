use crate::storage::Storage;
use crate::error::Error;
use heed::{DatabaseFlags, RwTxn};
use heed::types::UnalignedSlice;

impl Storage {
    pub(super) fn m12_migrate<'a>(&'a self, prefix: &str, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        // Trigger databases into existence

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

        // heed doesn't expose mdb_drop(1) yet, so we can't actually remove this database.

        Ok(())
    }

}
