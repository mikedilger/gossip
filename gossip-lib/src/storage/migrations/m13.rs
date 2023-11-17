use crate::storage::Storage;
use crate::storage::types::PersonList1;
use crate::error::Error;
use heed::RwTxn;
use nostr_types::PublicKey;
use std::collections::HashMap;

impl Storage {
    pub(super) fn m13_migrate<'a>(&'a self, prefix: &str, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        // Trigger databases into existence
        let _ = self.db_person_lists1()?;
        let _ = self.db_person_lists2()?;

        // Info message
        tracing::info!("{prefix}: migrating lists...");

        // Migrate
        self.m13_migrate_lists(txn)?;

        Ok(())
    }

    fn m13_migrate_lists<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let loop_txn = self.env.read_txn()?;
        for result in self.db_person_lists1()?.iter(&loop_txn)? {
            let (key, val) = result?;
            let pubkey = PublicKey::from_bytes(key, true)?;
            let mut person_lists = val
                .iter()
                .map(|u| PersonList1::from_u8(*u))
                .collect::<Vec<PersonList1>>();
            let new_person_lists: HashMap<PersonList1, bool> =
                person_lists.drain(..).map(|l| (l, true)).collect();

            self.write_person_lists2(&pubkey, new_person_lists, Some(txn))?;
        }

        // remove db_person_lists1
        {
            self.db_person_lists1()?.clear(txn)?;
            // heed doesn't expose mdb_drop(1) yet, so we can't actually remove this database.
        }

        Ok(())
    }

}
