use crate::error::Error;
use crate::storage::types::Person4;
use crate::storage::{AkciKey, Person3Table, Person4Table, Storage, Table};
use heed::{RoTxn, RwTxn};
use nostr_types::{EventKind, Id, PublicKey, Unixtime};
use std::ops::Bound;
use std::sync::OnceLock;

impl Storage {
    pub(super) fn m41_trigger(&self) -> Result<(), Error> {
        let _ = self.db_events()?;
        let _ = self.db_event_akci_index()?;
        let _ = Person3Table::db()?;
        let _ = Person4Table::db()?;
        Ok(())
    }

    pub(super) fn m41_migrate(&self, prefix: &str, txn: &mut RwTxn<'_>) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: Migrating person records...");

        // Migrate
        self.m41_migrate_person_records(txn)?;

        Ok(())
    }

    fn m41_migrate_person_records(&self, txn: &mut RwTxn<'_>) -> Result<(), Error> {
        let loop_txn = self.env.read_txn()?;

        let iter = Person3Table::iter(&loop_txn)?;
        for p in iter {
            let mut p4 = Person4 {
                pubkey: p.pubkey,
                first_encountered: self.m41_earliest_event_created_at(p.pubkey, &loop_txn)?.0,
                petname: p.petname,
                metadata_json: p.metadata_json,
                deserialized_metadata: OnceLock::new(),
                metadata_created_at: p.metadata_created_at,
                metadata_last_received: p.metadata_last_received,
                nip05_valid: p.nip05_valid,
                nip05_last_checked: p.nip05_last_checked,
                relay_list_created_at: p.relay_list_created_at,
                relay_list_last_sought: p.relay_list_last_sought,
                dm_relay_list_created_at: p.dm_relay_list_created_at,
                dm_relay_list_last_sought: p.dm_relay_list_last_sought,
            };
            Person4Table::write_record(&mut p4, Some(txn))?;
        }

        Person3Table::clear(Some(txn))?;

        Ok(())
    }

    fn m41_earliest_event_created_at(
        &self,
        author: PublicKey,
        txn: &RoTxn<'_>,
    ) -> Result<Unixtime, Error> {
        let mut oldest: Unixtime = Unixtime::now();

        let iter = {
            let start_prefix =
                AkciKey::from_parts(author, EventKind::Metadata, Unixtime(i64::MAX), Id([0; 32]));
            let end_prefix = AkciKey::from_parts(
                author,
                EventKind::Other(u32::MAX),
                Unixtime(0),
                Id([255; 32]),
            );
            let range = (
                Bound::Included(start_prefix.as_slice()),
                Bound::Excluded(end_prefix.as_slice()),
            );
            self.db_event_akci_index()?.range(txn, &range)?
        };
        for result in iter {
            let (keybytes, _) = result?;
            let key = AkciKey::from_bytes(keybytes)?;
            let (_, _, created_at, _) = key.into_parts()?;
            if created_at < oldest {
                oldest = created_at;
            }
        }

        Ok(oldest)
    }
}
