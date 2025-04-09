use crate::error::Error;
use crate::storage::types::Relay3;
use crate::storage::Storage;
use heed::RwTxn;

impl Storage {
    pub(super) fn m38_trigger(&self) -> Result<(), Error> {
        self.db_relays2()?;
        self.db_relays3()?;
        Ok(())
    }

    pub(super) fn m38_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: Migrating relay records...");

        // Migrate
        self.m38_migrate_relay_records(txn)?;

        Ok(())
    }

    fn m38_migrate_relay_records<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let mut old = self.filter_relays2(|_| true)?;
        for relay2 in old.drain(..) {
            let usage_bits = relay2.get_usage_bits();
            let relay3 = Relay3 {
                url: relay2.url,
                success_count: relay2.success_count,
                failure_count: relay2.failure_count,
                last_connected_at: relay2.last_connected_at,
                last_general_eose_at: relay2.last_general_eose_at,
                rank: relay2.rank,
                hidden: relay2.hidden,
                usage_bits,
                nip11: relay2.nip11,
                last_attempt_nip11: relay2.last_attempt_nip11,
                allow_connect: relay2.allow_connect,
                allow_auth: relay2.allow_auth,
                avoid_until: None,
            };
            self.write_relay3(&relay3, Some(txn))?;
        }

        // Clear the old database
        self.db_relays2()?.clear(txn)?;

        Ok(())
    }
}
