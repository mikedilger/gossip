use super::Storage;
use crate::error::{Error, ErrorKind};
use lmdb::{Cursor, RwTransaction, Transaction};
use nostr_types::Event;
use speedy::Readable;

mod settings;

impl Storage {
    const MAX_MIGRATION_LEVEL: u32 = 2;

    pub(super) fn migrate(&self, mut level: u32) -> Result<(), Error> {
        if level > Self::MAX_MIGRATION_LEVEL {
            return Err(ErrorKind::General(format!(
                "Migration level {} unknown: This client is older than your data.",
                level
            ))
            .into());
        }

        let mut txn = self.env.begin_rw_txn()?;
        while level < Self::MAX_MIGRATION_LEVEL {
            self.migrate_inner(level, &mut txn)?;
            level += 1;
            self.write_migration_level(level, Some(&mut txn))?;
        }
        txn.commit()?;

        Ok(())
    }

    fn migrate_inner<'a>(&'a self, level: u32, txn: &mut RwTransaction<'a>) -> Result<(), Error> {
        let prefix = format!("LMDB Migration {} -> {}", level, level + 1);
        match level {
            0 => {
                let total = self.get_event_stats()?.entries();
                tracing::info!(
                    "{prefix}: Computing and storing event relationships for {total} events..."
                );
                self.compute_relationships(total, Some(txn))?;
            }
            1 => {
                tracing::info!("{prefix}: Updating Settings...");
                self.try_migrate_settings1_settings2(Some(txn))?;
            }
            _ => panic!("Unreachable migration level"),
        };

        tracing::info!("done.");

        Ok(())
    }

    // Load and process every event in order to generate the relationships data
    fn compute_relationships<'a>(
        &'a self,
        total: usize,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        self.disable_sync()?;

        let f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            // track progress
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
                for checkpoint in &[10, 20, 30, 40, 50, 60, 70, 80, 90] {
                    if count == checkpoint * total / 100 {
                        tracing::info!("{}% done", checkpoint);
                    }
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
