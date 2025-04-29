use crate::error::Error;
use crate::storage::types::{PersonList1, PersonListMetadata3};
use crate::storage::Storage;
use heed::RwTxn;

impl Storage {
    pub(super) fn m0_trigger(&self) -> Result<(), Error> {
        self.db_person_lists_metadata3()?;
        Ok(())
    }

    pub(super) fn m0_migrate<'a>(&'a self, prefix: &str, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: Initializing Followed and Muted person lists...");

        // Migrate
        self.m0_initialize_default_person_lists(txn)?;

        Ok(())
    }

    fn m0_initialize_default_person_lists<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        {
            let metadata = PersonListMetadata3 {
                dtag: "muted".to_string(),
                title: "Muted".to_string(),
                ..Default::default()
            };

            self.set_person_list_metadata3(PersonList1::Muted, &metadata, Some(txn))?;
        }

        {
            let metadata = PersonListMetadata3 {
                dtag: "followed".to_owned(),
                title: "Followed".to_owned(),
                ..Default::default()
            };

            self.set_person_list_metadata3(PersonList1::Followed, &metadata, Some(txn))?;
        }

        Ok(())
    }
}
