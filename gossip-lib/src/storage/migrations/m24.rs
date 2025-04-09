use crate::error::Error;
use crate::storage::types::Relay2;
use crate::storage::Storage;
use heed::RwTxn;

impl Storage {
    pub(super) fn m24_trigger(&self) -> Result<(), Error> {
        self.db_relays1()?;
        self.db_relays2()?;
        Ok(())
    }

    pub(super) fn m24_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: Migrating Relay records...");

        // Migrate
        self.m24_migrate_relay_records(txn)?;

        Ok(())
    }

    fn m24_migrate_relay_records<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let mut old = self.filter_relays1(|_| true)?;
        for relay1 in old.drain(..) {
            let usage_bits = relay1.get_usage_bits();
            let relay2 = Relay2 {
                url: relay1.url,
                success_count: relay1.success_count,
                failure_count: relay1.failure_count,
                last_connected_at: relay1.last_connected_at,
                last_general_eose_at: relay1.last_general_eose_at,
                rank: relay1.rank,
                hidden: relay1.hidden,
                usage_bits,
                nip11: relay1.nip11,
                last_attempt_nip11: relay1.last_attempt_nip11,
                allow_connect: None,
                allow_auth: None,
            };
            self.write_relay2(&relay2, Some(txn))?;
        }

        // Clear the old database
        self.db_relays1()?.clear(txn)?;

        Ok(())
    }
}
