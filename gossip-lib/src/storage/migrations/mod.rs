// Migrations before m23 (except critical ones) are dropped from gossip-0.11
// so you must run gossip-0.9 or gossip-0.10 at least once to come up to
// m23 (or m28) first.

mod m19; // Creates person list metadata
mod m20; // Initializes person list metadata
mod m21; // Migrates person list metadata
mod m22; // Migrates person list metadata again
mod m23;
mod m24;
mod m25;
mod m26;
mod m27;
mod m28;
mod m29;
mod m30;
mod m31;
mod m32;
mod m33;
mod m34;
mod m35;
mod m36;
mod m37;
mod m38;
mod m39;
mod m40;
mod m41;
mod m42;

use super::Storage;
use crate::error::{Error, ErrorKind};
use heed::RwTxn;

impl Storage {
    const MIN_MIGRATION_LEVEL: u32 = 23;
    const MAX_MIGRATION_LEVEL: u32 = 42;

    /// Initialize the database from empty
    pub(super) fn init_from_empty(&self) -> Result<(), Error> {
        // Migrations that modify old data are not necessary here if we don't
        // have any old data.  These are migrations that create data or subsequently
        // modify that created data
        #[rustfmt::skip]
        let necessary: Vec<u32> = vec![
            19,  // Creates person list metadata
            20,  // Initializes person list metadata
            21,  // Migrates person list metadata
            22,  // Migrates person list metadata again
        ];

        for level in necessary.iter() {
            self.trigger(*level)?;
            let mut txn = self.env.write_txn()?;
            self.migrate_inner(*level, &mut txn)?;
            self.write_migration_level(*level, Some(&mut txn))?;
            txn.commit()?;
        }

        let mut txn = self.env.write_txn()?;
        self.write_migration_level(Self::MAX_MIGRATION_LEVEL, Some(&mut txn))?;
        txn.commit()?;

        Ok(())
    }

    pub(super) fn migrate(&self, mut level: u32) -> Result<(), Error> {
        if level < Self::MIN_MIGRATION_LEVEL {
            let lmdb_dir = crate::profile::Profile::lmdb_dir()
                .map_or("<notfound>".to_owned(), |p| format!("{}/", p.display()));
            eprintln!("DATABASE IS TOO OLD");
            eprintln!("-------------------");
            eprintln!(
                "This version of gossip cannot handle your old database. You have two options:"
            );
            eprintln!("Option 1: Run gossip 0.9 or 0.10 at least once to upgrade, or");
            eprintln!(
                "Option 2: Delete your database directory {} and restart to start fresh",
                lmdb_dir
            );
            return Err(
                ErrorKind::General(format!("Migration level {} is too old.", level)).into(),
            );
        }

        if level > Self::MAX_MIGRATION_LEVEL {
            return Err(ErrorKind::General(format!(
                "Migration level {} unknown: This client is older than your data.",
                level
            ))
            .into());
        }

        while level < Self::MAX_MIGRATION_LEVEL {
            level += 1;
            self.trigger(level)?;
            let mut txn = self.env.write_txn()?;
            self.migrate_inner(level, &mut txn)?;
            self.write_migration_level(level, Some(&mut txn))?;
            txn.commit()?;
        }

        Ok(())
    }

    fn trigger(&self, level: u32) -> Result<(), Error> {
        match level {
            19 => self.m19_trigger()?,
            20 => self.m20_trigger()?,
            21 => self.m21_trigger()?,
            22 => self.m22_trigger()?,
            23 => self.m23_trigger()?,
            24 => self.m24_trigger()?,
            25 => self.m25_trigger()?,
            26 => self.m26_trigger()?,
            27 => self.m27_trigger()?,
            28 => self.m28_trigger()?,
            29 => self.m29_trigger()?,
            30 => self.m30_trigger()?,
            31 => self.m31_trigger()?,
            32 => self.m32_trigger()?,
            33 => self.m33_trigger()?,
            34 => self.m34_trigger()?,
            35 => self.m35_trigger()?,
            36 => self.m36_trigger()?,
            37 => self.m37_trigger()?,
            38 => self.m38_trigger()?,
            39 => self.m39_trigger()?,
            40 => self.m40_trigger()?,
            41 => self.m41_trigger()?,
            42 => self.m42_trigger()?,
            _ => panic!("Unreachable migration level"),
        }

        Ok(())
    }

    fn migrate_inner<'a>(&'a self, level: u32, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let prefix = format!("LMDB Migration {}", level);
        match level {
            19 => self.m19_migrate(&prefix, txn)?,
            20 => self.m20_migrate(&prefix, txn)?,
            21 => self.m21_migrate(&prefix, txn)?,
            22 => self.m22_migrate(&prefix, txn)?,
            23 => self.m23_migrate(&prefix, txn)?,
            24 => self.m24_migrate(&prefix, txn)?,
            25 => self.m25_migrate(&prefix, txn)?,
            26 => self.m26_migrate(&prefix, txn)?,
            27 => self.m27_migrate(&prefix, txn)?,
            28 => self.m28_migrate(&prefix, txn)?,
            29 => self.m29_migrate(&prefix, txn)?,
            30 => self.m30_migrate(&prefix, txn)?,
            31 => self.m31_migrate(&prefix, txn)?,
            32 => self.m32_migrate(&prefix, txn)?,
            33 => self.m33_migrate(&prefix, txn)?,
            34 => self.m34_migrate(&prefix, txn)?,
            35 => self.m35_migrate(&prefix, txn)?,
            36 => self.m36_migrate(&prefix, txn)?,
            37 => self.m37_migrate(&prefix, txn)?,
            38 => self.m38_migrate(&prefix, txn)?,
            39 => self.m39_migrate(&prefix, txn)?,
            40 => self.m40_migrate(&prefix, txn)?,
            41 => self.m41_migrate(&prefix, txn)?,
            42 => self.m42_migrate(&prefix, txn)?,
            _ => panic!("Unreachable migration level"),
        };

        tracing::info!("done.");

        Ok(())
    }
}
