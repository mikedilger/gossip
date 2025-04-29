// Migrations before m26 are dropped from gossip-0.15
// so you must run gossip-0.14 at least once to come up to m26.

mod m0; // initial setup, this must include all initialization up to MAX level

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
mod m43;
mod m44;
mod m45;
mod m46;
mod m47;

use super::Storage;
use crate::error::{Error, ErrorKind};
use heed::RwTxn;

impl Storage {
    const MIN_MIGRATION_LEVEL: u32 = 25;
    const MAX_MIGRATION_LEVEL: u32 = 47;

    /// Initialize the database from empty
    pub(super) async fn init_from_empty(&self) -> Result<(), Error> {
        self.trigger(0)?;
        let mut txn = self.env.write_txn()?;
        self.migrate_inner(0, &mut txn).await?;
        self.write_migration_level(Self::MAX_MIGRATION_LEVEL, Some(&mut txn))?;
        txn.commit()?;

        Ok(())
    }

    pub(super) async fn migrate(&self, mut level: u32) -> Result<(), Error> {
        if level < Self::MIN_MIGRATION_LEVEL {
            let lmdb_dir = crate::profile::Profile::lmdb_dir()
                .map_or("<notfound>".to_owned(), |p| format!("{}/", p.display()));
            eprintln!("DATABASE IS TOO OLD");
            eprintln!("-------------------");
            eprintln!(
                "This version of gossip cannot handle your old database. You have two options:"
            );
            eprintln!("Option 1: Run gossip 0.14 at least once to upgrade, or");
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
            self.migrate_inner(level, &mut txn).await?;
            self.write_migration_level(level, Some(&mut txn))?;
            txn.commit()?;
        }

        Ok(())
    }

    fn trigger(&self, level: u32) -> Result<(), Error> {
        match level {
            0 => self.m0_trigger()?,
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
            43 => self.m43_trigger()?,
            44 => self.m44_trigger()?,
            45 => self.m45_trigger()?,
            46 => self.m46_trigger()?,
            47 => self.m47_trigger()?,
            _ => panic!("Unreachable migration level"),
        }

        Ok(())
    }

    async fn migrate_inner<'a>(&'a self, level: u32, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let prefix = format!("LMDB Migration {}", level);
        match level {
            0 => self.m0_migrate(&prefix, txn)?,
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
            43 => self.m43_migrate(&prefix, txn)?,
            44 => self.m44_migrate(&prefix, txn)?,
            45 => self.m45_migrate(&prefix, txn)?,
            46 => self.m46_migrate(&prefix, txn)?,
            47 => self.m47_migrate(&prefix, txn)?,
            _ => panic!("Unreachable migration level"),
        };

        tracing::info!("done.");

        Ok(())
    }
}
