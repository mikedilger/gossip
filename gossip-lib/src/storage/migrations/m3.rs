use crate::storage::Storage;
use crate::error::Error;
use heed::RwTxn;
use nostr_types::RelayUrl;

impl Storage {
    pub(super) fn m3_migrate<'a>(&'a self, prefix: &str, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        // Trigger databases into existence
        let _ = self.db_relays1()?;

        // Info message
        tracing::info!("{prefix}: Removing invalid relays...");

        // Migrate
        self.m3_remove_invalid_relays(txn)?;

        Ok(())
    }

    fn m3_remove_invalid_relays<'a>(&'a self, rw_txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let bad_relays =
            self.filter_relays1(|relay| RelayUrl::try_from_str(relay.url.as_str()).is_err())?;

        for relay in &bad_relays {
            tracing::info!("Deleting bad relay: {}", relay.url);

            self.delete_relay1(&relay.url, Some(rw_txn))?;
        }

        tracing::info!("Deleted {} bad relays", bad_relays.len());

        Ok(())
    }

}
