use heed::RwTxn;

use crate::error::Error;
use crate::storage::Storage;

impl Storage {
    pub(super) fn m27_trigger(&self) -> Result<(), Error> {
        Ok(())
    }

    pub(super) fn m27_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: deleting old nostr-connect data (you can set it up again)...");

        // Migrate
        self.m27_migrate_delete_nostr_connect(txn)?;

        Ok(())
    }

    fn m27_migrate_delete_nostr_connect<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        // Delete the unconnected server
        self.delete_nip46_unconnected_server(Some(txn))?;

        // Clear all servers
        self.db_nip46servers()?.clear(txn)?;

        Ok(())
    }
}
