use crate::error::Error;
use crate::storage::Storage;
use heed::RwTxn;

impl Storage {
    pub(super) fn m23_trigger(&self) -> Result<(), Error> {
        let _ = self.db_person_lists_metadata3()?;
        Ok(())
    }

    pub(super) fn m23_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: fixing list counts...");

        // Migrate
        self.m23_recount_list_lengths(txn)?;

        Ok(())
    }

    fn m23_recount_list_lengths<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        for (list, mut metadata) in self.get_all_person_list_metadata3()? {
            let people = self.get_people_in_list2(list)?;
            metadata.len = people.len();
            self.set_person_list_metadata3(list, &metadata, Some(txn))?;
        }

        Ok(())
    }
}
