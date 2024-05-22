use crate::error::Error;
use crate::storage::types::{PersonRelay1, PersonRelay2};
use crate::storage::Storage;
use heed::RwTxn;
use speedy::{Readable, Writable};

impl Storage {
    pub(super) fn m34_trigger(&self) -> Result<(), Error> {
        let _ = self.db_person_relays1()?;
        let _ = self.db_person_relays2()?;
        Ok(())
    }

    pub(super) fn m34_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: Migrating person_relay records...");

        // Migrate
        self.m34_migrate_person_relay_records(txn)?;

        Ok(())
    }

    fn m34_migrate_person_relay_records<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let loop_txn = self.env.read_txn()?;
        let iter = self.db_person_relays1()?.iter(&loop_txn)?;
        for result in iter {
            let (key, val) = result?;
            let pr = PersonRelay1::read_from_buffer(val)?;
            let last_suggested = match (pr.last_suggested_kind3, pr.last_suggested_bytag) {
                (None, None) => None,
                (None, Some(b)) => Some(b),
                (Some(a), None) => Some(a),
                (Some(a), Some(b)) => Some(a.max(b)),
            };
            let pr2 = PersonRelay2 {
                pubkey: pr.pubkey,
                url: pr.url,
                read: pr.read || pr.last_suggested_nip05.is_some(),
                write: pr.write || pr.last_suggested_nip05.is_some(),
                dm: false,
                last_fetched: pr.last_fetched,
                last_suggested,
            };
            let bytes = pr2.write_to_vec()?;
            self.db_person_relays2()?.put(txn, key, &bytes)?;
        }

        self.db_person_relays1()?.clear(txn)?;

        Ok(())
    }
}
