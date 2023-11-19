use crate::error::Error;
use crate::storage::types::PersonList1;
use crate::storage::Storage;
use heed::RwTxn;

impl Storage {
    pub(super) fn m6_trigger(&self) -> Result<(), Error> {
        let _ = self.db_people1()?;
        let _ = self.db_person_lists1()?;
        Ok(())
    }

    pub(super) fn m6_migrate<'a>(&'a self, prefix: &str, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: populating new lists...");

        // Migrate
        self.m6_populate_new_lists(txn)?;

        Ok(())
    }

    fn m6_populate_new_lists<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let mut count: usize = 0;
        let mut followed_count: usize = 0;
        for person1 in self.filter_people1(|_| true)?.iter() {
            let mut lists: Vec<PersonList1> = Vec::new();
            if person1.followed {
                lists.push(PersonList1::Followed);
                followed_count += 1;
            }
            if person1.muted {
                lists.push(PersonList1::Muted);
            }
            if !lists.is_empty() {
                self.write_person_lists1(&person1.pubkey, lists, Some(txn))?;
                count += 1;
            }
        }

        tracing::info!(
            "{} people added to new lists, {} followed",
            count,
            followed_count
        );

        // This migration does not remove the old data. The next one will.
        Ok(())
    }
}
