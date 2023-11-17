use crate::storage::Storage;
use crate::storage::types::PersonList1;
use crate::error::Error;
use heed::RwTxn;
use std::collections::HashMap;

impl Storage {
    pub(super) fn m14_migrate<'a>(&'a self, prefix: &str, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        // Trigger databases into existence

        // Info message
        tracing::info!("{prefix}: moving person list last edit times...");

        // Migrate
        self.m14_move_person_list_last_edit_times(txn)?;

        Ok(())
    }

    fn m14_move_person_list_last_edit_times<'a>(
        &'a self,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        let mut edit_times: HashMap<PersonList1, i64> = HashMap::new();
        edit_times.insert(PersonList1::Followed, self.read_last_contact_list_edit()?);
        edit_times.insert(PersonList1::Muted, self.read_last_mute_list_edit()?);
        self.write_person_lists_last_edit_times(edit_times, Some(txn))?;
        Ok(())
    }

}
