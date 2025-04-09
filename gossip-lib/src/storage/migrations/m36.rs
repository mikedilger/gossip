use crate::error::Error;
use crate::storage::types::{PersonRelay2, Relay2};
use crate::storage::Storage;
use heed::RwTxn;
use speedy::Readable;

impl Storage {
    pub(super) fn m36_trigger(&self) -> Result<(), Error> {
        self.db_relays2()?;
        self.db_person_relays2()?;
        Ok(())
    }

    pub(super) fn m36_migrate(&self, prefix: &str, txn: &mut RwTxn<'_>) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: Cleaning out relays...");

        // Migrate
        self.m36_clean_out_relays(txn)?;

        Ok(())
    }

    fn m36_clean_out_relays(&self, txn: &mut RwTxn<'_>) -> Result<(), Error> {
        // Delete any person_relay with this relay
        let mut deletions: Vec<Vec<u8>> = Vec::new();
        for result in self.db_person_relays2()?.iter(txn)? {
            let (key, val) = result?;
            if let Ok(person_relay) = PersonRelay2::read_from_buffer(val) {
                if Self::url_is_banned(&person_relay.url) {
                    deletions.push(key.to_owned());
                }
            }
        }
        for deletion in deletions.drain(..) {
            self.db_person_relays2()?.delete(txn, &deletion)?;
        }

        // Delete any relay that matches
        let mut deletions: Vec<Vec<u8>> = Vec::new();
        {
            for result in self.db_relays2()?.iter(txn)? {
                let (key, val) = result?;
                let relay: Relay2 = serde_json::from_slice(val)?;
                if Self::url_is_banned(&relay.url) {
                    deletions.push(key.to_owned());
                }
            }
        }
        for deletion in deletions.drain(..) {
            self.db_relays2()?.delete(txn, &deletion)?;
        }

        Ok(())
    }
}
