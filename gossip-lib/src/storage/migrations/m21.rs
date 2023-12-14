use crate::error::Error;
use crate::storage::types::PersonListMetadata2;
use crate::storage::Storage;
use heed::RwTxn;

impl Storage {
    pub(super) fn m21_trigger(&self) -> Result<(), Error> {
        let _ = self.db_person_lists_metadata1()?;
        let _ = self.db_person_lists_metadata2()?;
        Ok(())
    }

    pub(super) fn m21_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: ...");

        // Migrate
        self.m21_migrate_person_list_metadata(txn)?;

        Ok(())
    }

    fn m21_migrate_person_list_metadata<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let mut old = self.get_all_person_list_metadata1()?;
        for (list, metadata1) in old.drain(..) {
            let metadata2 = PersonListMetadata2 {
                dtag: metadata1.dtag,
                title: metadata1.title,
                last_edit_time: metadata1.last_edit_time,
                event_created_at: metadata1.event_created_at,
                event_public_len: metadata1.event_public_len,
                event_private_len: metadata1.event_private_len,
                ..Default::default()
            };
            self.set_person_list_metadata2(list, &metadata2, Some(txn))?;
        }

        // Clear the old database
        self.db_person_lists_metadata1()?.clear(txn)?;

        Ok(())
    }
}
