mod legacy;
use super::Storage;
use crate::error::Error;
use nostr_types::EncryptedPrivateKey;
use rusqlite::Connection;

impl Storage {
    pub(super) fn import(&self) -> Result<(), Error> {
        tracing::info!("Importing SQLITE data into LMDB...");

        // Progress the legacy database to the endpoint first
        let mut db = legacy::init_database()?;
        legacy::setup_database(&mut db)?;
        tracing::info!("LDMB: setup");

        // local settings
        import_local_settings(&db, |epk: Option<EncryptedPrivateKey>, lcle: i64| {
            self.write_encrypted_private_key(&epk)?;
            self.write_last_contact_list_edit(lcle)
        })?;

        // Mark migration level
        // TBD: self.write_migration_level(0)?;

        tracing::info!("Importing SQLITE data into LMDB: Done.");

        Ok(())
    }
}

fn import_local_settings<F>(db: &Connection, mut f: F) -> Result<(), Error>
where
    F: FnMut(Option<EncryptedPrivateKey>, i64) -> Result<(), Error>,
{
    // These are the only local settings we need to keep
    let sql = "SELECT encrypted_private_key, last_contact_list_edit FROM local_settings";
    let mut stmt = db.prepare(sql)?;
    let mut rows = stmt.raw_query();
    if let Some(row) = rows.next()? {
        let epk: Option<String> = row.get(0)?;
        let lcle: i64 = row.get(1)?;
        f(epk.map(EncryptedPrivateKey), lcle)?;
    }
    Ok(())
}
