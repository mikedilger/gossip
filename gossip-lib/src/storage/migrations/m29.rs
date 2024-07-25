use crate::error::Error;
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::Event;
use speedy::Readable;

impl Storage {
    pub(super) fn m29_trigger(&self) -> Result<(), Error> {
        Ok(())
    }

    pub(super) fn m29_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: Building new event indexes...");

        // Migrate
        self.m29_build_new_event_indexes(txn)?;

        Ok(())
    }

    fn m29_build_new_event_indexes<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let loop_txn = self.env().read_txn()?;
        for result in self.db_events()?.iter(&loop_txn)? {
            let (_, bytes) = result?;
            let event = Event::read_from_buffer(bytes)?;
            self.write_event_akci_index(
                event.pubkey,
                event.kind,
                event.created_at,
                event.id,
                Some(txn),
            )?;
            self.write_event_kci_index(event.kind, event.created_at, event.id, Some(txn))?;
        }
        Ok(())
    }
}
