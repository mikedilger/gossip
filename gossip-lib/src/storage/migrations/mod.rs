mod deprecated;

mod m1;
mod m10;
mod m11;
mod m12;
mod m13;
mod m14;
mod m15;
mod m16;
mod m17;
mod m18;
mod m19;
mod m2;
mod m20;
mod m21;
mod m22;
mod m23;
mod m24;
mod m25;
mod m26;
mod m27;
mod m28;
mod m29;
mod m3;
mod m30;
mod m31;
mod m32;
mod m4;
mod m5;
mod m6;
mod m7;
mod m8;
mod m9;

use super::Storage;
use crate::error::{Error, ErrorKind};
use heed::RwTxn;

impl Storage {
    const MAX_MIGRATION_LEVEL: u32 = 32;

    /// Initialize the database from empty
    pub(super) fn init_from_empty(&self) -> Result<(), Error> {
        // Migrations that modify old data are not necessary here if we don't
        // have any old data.  These are migrations that create data or subsequently
        // modify that created data
        #[rustfmt::skip]
        let necessary: Vec<u32> = vec![
            6,   // Creates Followed and Muted default person lists
            13,  // Migrates Person Lists
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
            1 => self.m1_trigger()?,
            2 => self.m2_trigger()?,
            3 => self.m3_trigger()?,
            4 => self.m4_trigger()?,
            5 => self.m5_trigger()?,
            6 => self.m6_trigger()?,
            7 => self.m7_trigger()?,
            8 => self.m8_trigger()?,
            9 => self.m9_trigger()?,
            10 => self.m10_trigger()?,
            11 => self.m11_trigger()?,
            12 => self.m12_trigger()?,
            13 => self.m13_trigger()?,
            14 => self.m14_trigger()?,
            15 => self.m15_trigger()?,
            16 => self.m16_trigger()?,
            17 => self.m17_trigger()?,
            18 => self.m18_trigger()?,
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
            _ => panic!("Unreachable migration level"),
        }

        Ok(())
    }

    fn migrate_inner<'a>(&'a self, level: u32, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let prefix = format!("LMDB Migration {}", level);
        match level {
            1 => self.m1_migrate(&prefix, txn)?,
            2 => self.m2_migrate(&prefix, txn)?,
            3 => self.m3_migrate(&prefix, txn)?,
            4 => self.m4_migrate(&prefix, txn)?,
            5 => self.m5_migrate(&prefix, txn)?,
            6 => self.m6_migrate(&prefix, txn)?,
            7 => self.m7_migrate(&prefix, txn)?,
            8 => self.m8_migrate(&prefix, txn)?,
            9 => self.m9_migrate(&prefix, txn)?,
            10 => self.m10_migrate(&prefix, txn)?,
            11 => self.m11_migrate(&prefix, txn)?,
            12 => self.m12_migrate(&prefix, txn)?,
            13 => self.m13_migrate(&prefix, txn)?,
            14 => self.m14_migrate(&prefix, txn)?,
            15 => self.m15_migrate(&prefix, txn)?,
            16 => self.m16_migrate(&prefix, txn)?,
            17 => self.m17_migrate(&prefix, txn)?,
            18 => self.m18_migrate(&prefix, txn)?,
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
            _ => panic!("Unreachable migration level"),
        };

        tracing::info!("done.");

        Ok(())
    }
}
