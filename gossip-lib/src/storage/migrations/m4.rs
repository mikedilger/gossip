use crate::storage::Storage;
use crate::error::Error;
use heed::RwTxn;
use nostr_types::{EventV1, Id, Signature};
use speedy::Readable;

impl Storage {
    pub(super) fn m4_migrate<'a>(&'a self, prefix: &str, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        // Trigger databases into existence
        let _ = self.db_events1()?;

        // Info message
        tracing::info!("{prefix}: deleting decrypted rumors...");

        // Migrate
        self.m4_delete_rumors(txn)?;

        Ok(())
    }

    fn m4_delete_rumors<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let mut ids: Vec<Id> = Vec::new();
        let iter = self.db_events1()?.iter(txn)?;
        for result in iter {
            let (_key, val) = result?;
            let event = EventV1::read_from_buffer(val)?;
            if event.sig == Signature::zeroes() {
                ids.push(event.id);
            }
        }

        for id in ids {
            self.db_events1()?.delete(txn, id.as_slice())?;
        }

        Ok(())
    }

}


