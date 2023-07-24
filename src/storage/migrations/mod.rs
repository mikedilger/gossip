use super::Storage;
use crate::error::{Error, ErrorKind};
use lmdb::{Cursor, Transaction};
use nostr_types::Event;
use speedy::Readable;

impl Storage {
    const MIGRATION_LEVEL: u32 = 1;

    pub(super) fn migrate(&self, mut level: u32) -> Result<(), Error> {
        if level > Self::MIGRATION_LEVEL {
            return Err(ErrorKind::General(format!(
                "Migration level {} unknown: This client is older than your data.",
                level
            ))
            .into());
        }

        while level < Self::MIGRATION_LEVEL {
            level += 1;
            tracing::info!("LMDB Migration to level {}...", level);

            self.migrate_inner(level)?;

            self.write_migration_level(level)?;
        }

        Ok(())
    }

    fn migrate_inner(&self, level: u32) -> Result<(), Error> {
        match level {
            0 => Ok(()),
            1 => self.compute_relationships(),
            n => panic!("Unknown migration level {}", n),
        }
    }

    // Load and process every event in order to generate the relationships data
    fn compute_relationships(&self) -> Result<(), Error> {
        // track progress
        let total = self.get_event_stats()?.entries();
        let mut count = 0;

        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.events)?;
        let iter = cursor.iter_start();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((_key, val)) => {
                    let event = Event::read_from_buffer(val)?;
                    let _ = self.process_relationships_of_event(&event)?;
                }
            }

            // track progress
            count += 1;
            if count % 1000 == 0 {
                tracing::info!("{}/{}", count, total);
            }
        }

        Ok(())
    }
}
