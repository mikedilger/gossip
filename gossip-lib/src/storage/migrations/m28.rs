use heed::RwTxn;

use crate::error::Error;
use crate::storage::types::Person2;
use crate::storage::Storage;

impl Storage {
    pub(super) fn m28_trigger(&self) -> Result<(), Error> {
        let _ = self.db_people2()?;
        Ok(())
    }

    pub(super) fn m28_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: fixing empty petnames...");

        // Migrate
        self.m28_fix_empty_petnames(txn)?;

        Ok(())
    }

    fn m28_fix_empty_petnames<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let loop_txn = self.env.read_txn()?;
        for result in self.db_people2()?.iter(&loop_txn)? {
            let (key, val) = result?;
            let mut person: Person2 = serde_json::from_slice(val)?;
            if person.petname == Some("".to_string()) {
                person.petname = None;
                let bytes = serde_json::to_vec(&person)?;
                self.db_people2()?.put(txn, key, &bytes)?;
            }
        }
        Ok(())
    }
}
