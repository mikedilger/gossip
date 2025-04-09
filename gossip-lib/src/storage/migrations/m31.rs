use crate::error::Error;
use crate::misc::Private;
use crate::storage::types::PersonList1;
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::PublicKey;
use speedy::Readable;
use std::collections::HashMap;

impl Storage {
    pub(super) fn m31_trigger(&self) -> Result<(), Error> {
        self.db_person_lists2()?;
        Ok(())
    }

    pub(super) fn m31_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: Changing sense of person lists public/private flag...");

        // Migrate
        self.m31_change_sense_of_person_lists_private(txn)?;

        Ok(())
    }

    fn m31_change_sense_of_person_lists_private<'a>(
        &'a self,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        let loop_txn = self.env.read_txn()?;
        for result in self.db_person_lists2()?.iter(&loop_txn)? {
            let (key, val) = result?;
            let pubkey = PublicKey::from_bytes(key, true)?;
            let mut map = HashMap::<PersonList1, bool>::read_from_buffer(val)?;
            let map2: HashMap<PersonList1, Private> =
                map.drain().map(|(k, v)| (k, Private(!v))).collect();
            self.write_person_lists2(&pubkey, map2, Some(txn))?;
        }

        Ok(())
    }
}
