use super::Storage;
use crate::error::{Error, ErrorKind};
use lmdb::{Cursor, RwTransaction, Transaction};
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

        let mut txn = self.env.begin_rw_txn()?;

        while level < Self::MIGRATION_LEVEL {
            level += 1;
            tracing::info!("LMDB Migration to level {}...", level);
            self.migrate_inner(level, Some(&mut txn))?;
            self.write_migration_level(level, Some(&mut txn))?;
        }

        txn.commit()?;

        Ok(())
    }

    fn migrate_inner<'a>(
        &'a self,
        level: u32,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        match level {
            0 => Ok(()),
            1 => self.compute_relationships(rw_txn),
            n => panic!("Unknown migration level {}", n),
        }
    }

    // Load and process every event in order to generate the relationships data
    fn compute_relationships<'a>(
        &'a self,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        self.disable_sync()?;

        let f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            // track progress
            let total = self.get_event_stats()?.entries();
            let mut count = 0;

            let event_txn = self.env.begin_ro_txn()?;
            let mut cursor = event_txn.open_ro_cursor(self.events)?;
            let iter = cursor.iter_start();
            for result in iter {
                match result {
                    Err(e) => return Err(e.into()),
                    Ok((_key, val)) => {
                        let event = Event::read_from_buffer(val)?;
                        let _ = self.process_relationships_of_event(&event, Some(txn))?;
                    }
                }

                // track progress
                count += 1;
                if count % 2000 == 0 {
                    tracing::info!("{}/{}", count, total);
                }
            }

            tracing::info!("syncing...");

            Ok(())
        };

        match rw_txn {
            Some(txn) => {
                f(txn)?;
            }
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        self.enable_sync()?;

        Ok(())
    }
}
