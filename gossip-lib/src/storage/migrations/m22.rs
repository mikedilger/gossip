use crate::error::Error;
use crate::misc::Private;
use crate::storage::types::PersonListMetadata3;
use crate::storage::Storage;
use heed::RwTxn;

impl Storage {
    pub(super) fn m22_trigger(&self) -> Result<(), Error> {
        let _ = self.db_person_lists_metadata2()?;
        let _ = self.db_person_lists_metadata3()?;
        Ok(())
    }

    pub(super) fn m22_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: migrating person list metadata (again)...");

        // Migrate
        self.m22_migrate_person_list_metadata(txn)?;

        Ok(())
    }

    fn m22_migrate_person_list_metadata<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let mut old = self.get_all_person_list_metadata2()?;
        for (list, metadata2) in old.drain(..) {
            let people = self.get_people_in_list2(list)?;
            let metadata3 = PersonListMetadata3 {
                dtag: metadata2.dtag,
                title: metadata2.title,
                last_edit_time: metadata2.last_edit_time,
                event_created_at: metadata2.event_created_at,
                event_public_len: metadata2.event_public_len,
                event_private_len: metadata2.event_private_len,
                favorite: metadata2.favorite,
                order: metadata2.order,
                private: Private(metadata2.private),
                len: people.len(),
            };
            self.set_person_list_metadata3(list, &metadata3, Some(txn))?;
        }

        // Clear the old database
        self.db_person_lists_metadata2()?.clear(txn)?;

        Ok(())
    }
}
