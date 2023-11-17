use crate::storage::Storage;
use crate::storage::types::{Settings1, Settings2};
use crate::error::{Error, ErrorKind};
use heed::RwTxn;
use speedy::{Readable, Writable};

impl Storage {
    pub(super) fn m2_trigger(&self) -> Result<(), Error> {
        let _ = self.db_events1()?;
        Ok(())
    }

    pub(super) fn m2_migrate<'a>(&'a self, prefix: &str, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: Updating Settings...");

        // Migrate
        self.m2_try_migrate_settings1_settings2(txn)?;

        Ok(())
    }

    fn m2_try_migrate_settings1_settings2<'a>(
        &'a self,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {

        // If something is under the old "settings" key
        if let Ok(Some(bytes)) = self.general.get(txn, b"settings") {
            let settings1 = match Settings1::read_from_buffer(bytes) {
                Ok(s1) => s1,
                Err(_) => {
                    tracing::error!("Settings are not deserializing. This is probably a code issue (although I have not found the bug yet). The best I can do is reset your settings to the default. This is better than the other option of wiping your entire database and starting over.");
                    Settings1::default()
                }
            };

            // Convert it to the new Settings2 structure
            let settings2: Settings2 = settings1.into();
            let bytes = settings2.write_to_vec()?;

            // And store it under the new "settings2" key
            self.general.put(txn, b"settings2", &bytes)?;

            // Then delete the old "settings" key
            self.general.delete(txn, b"settings")?;
        } else {
            return Err(ErrorKind::General("Settings missing.".to_string()).into());
        }

        Ok(())
    }
}
