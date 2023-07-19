
mod legacy;
use super::Storage;
use crate::error::Error;

impl Storage {
    pub(super) fn import(&self) -> Result<(), Error> {
        tracing::info!("Importing SQLITE data into LMDB...");

        // Progress the legacy database to the endpoint first
        let mut db = legacy::init_database()?;
        legacy::setup_database(&mut db)?;
        tracing::info!("LDMB: setup");

        // TBD: import here

        // Mark migration level
        // TBD: self.write_migration_level(0)?;

        tracing::info!("Importing SQLITE data into LMDB: Done.");

        Ok(())
    }
}
