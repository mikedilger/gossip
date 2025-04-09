use crate::error::Error;
use crate::storage::types::{Person2, Person3};
use crate::storage::{Person3Table, Storage, Table};
use heed::RwTxn;
use std::sync::OnceLock;

impl Storage {
    pub(super) fn m35_trigger(&self) -> Result<(), Error> {
        self.db_people2()?;
        Person3Table::db()?;
        Ok(())
    }

    pub(super) fn m35_migrate(&self, prefix: &str, txn: &mut RwTxn<'_>) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: Migrating person records...");

        // Migrate
        self.m35_migrate_person_records(txn)?;

        Ok(())
    }

    fn m35_migrate_person_records(&self, txn: &mut RwTxn<'_>) -> Result<(), Error> {
        let loop_txn = self.env.read_txn()?;
        let iter = self.db_people2()?.iter(&loop_txn)?;
        for result in iter {
            let (_key, val) = result?;
            let p: Person2 = serde_json::from_slice(val)?;
            let mut p3 = Person3 {
                pubkey: p.pubkey,
                petname: p.petname,
                metadata_json: match p.metadata {
                    None => None,
                    Some(inner) => Some(serde_json::to_string(&inner)?),
                },
                deserialized_metadata: OnceLock::new(),
                metadata_created_at: p.metadata_created_at,
                metadata_last_received: p.metadata_last_received,
                nip05_valid: p.nip05_valid,
                nip05_last_checked: p.nip05_last_checked,
                relay_list_created_at: p.relay_list_created_at,
                relay_list_last_sought: p.relay_list_last_sought,
                dm_relay_list_created_at: None,
                dm_relay_list_last_sought: 0,
            };
            Person3Table::write_record(&mut p3, Some(txn))?;
        }

        self.db_people2()?.clear(txn)?;

        Ok(())
    }
}
