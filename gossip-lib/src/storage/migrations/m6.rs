use crate::storage::Storage;
use crate::storage::types::Person2;
use crate::error::Error;
use heed::RwTxn;

impl Storage {
    pub(super) fn m6_migrate<'a>(&'a self, prefix: &str, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        // Trigger databases into existence
        let _ = self.db_people1()?;
        let _ = self.db_people2()?;

        // Info message
        tracing::info!("{prefix}: migrating person records...");

        // Migrate
        self.m6_migrate_people(txn)?;

        Ok(())
    }

    fn m6_migrate_people<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let mut count: usize = 0;
        for person1 in self.filter_people1(|_| true)?.drain(..) {
            let person2 = Person2 {
                pubkey: person1.pubkey,
                petname: person1.petname,
                metadata: person1.metadata,
                metadata_created_at: person1.metadata_created_at,
                metadata_last_received: person1.metadata_last_received,
                nip05_valid: person1.nip05_valid,
                nip05_last_checked: person1.nip05_last_checked,
                relay_list_created_at: person1.relay_list_created_at,
                relay_list_last_received: person1.relay_list_last_received,
            };
            self.write_person2(&person2, Some(txn))?;
            count += 1;
        }

        tracing::info!("Migrated {} people", count);

        // delete people1 database
        self.db_people1()?.clear(txn)?;
        // self.general.delete(txn, b"people")?; // LMDB doesn't allow this.

        Ok(())
    }
}
