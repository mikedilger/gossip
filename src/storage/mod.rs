mod import;

use crate::error::Error;
use crate::profile::Profile;
use crate::settings::Settings;
use lmdb::{Database, DatabaseFlags, Environment, EnvironmentFlags, Transaction, WriteFlags};
use nostr_types::EncryptedPrivateKey;
use speedy::{Readable, Writable};

const MAX_LMDB_KEY: usize = 511;

pub struct Storage {
    env: Environment,

    // General database (settings, local_settings)
    general: Database,
}

impl Storage {
    pub fn new() -> Result<Storage, Error> {
        let mut builder = Environment::new();

        builder.set_flags(
            EnvironmentFlags::WRITE_MAP | // no nested transactions!
            EnvironmentFlags::NO_META_SYNC |
            EnvironmentFlags::MAP_ASYNC,
        );
        // builder.set_max_readers(126); // this is the default
        builder.set_max_dbs(32);

        // This has to be big enough for all the data.
        // Note that it is the size of the map in VIRTUAL address space,
        //   and that it doesn't all have to be paged in at the same time.
        builder.set_map_size(1048576 * 1024 * 2); // 2 GB (probably too small)

        let env = builder.open(&Profile::current()?.lmdb_dir)?;

        let general = env.create_db(None, DatabaseFlags::empty())?;

        let storage = Storage { env, general };

        // If migration level is missing, we need to import from legacy sqlite
        match storage.read_migration_level()? {
            None => {
                // Import from sqlite
                storage.import()?;
            }
            Some(_level) => {
                // migrations happen here
            }
        }

        Ok(storage)
    }

    pub fn write_migration_level(&self, migration_level: u32) -> Result<(), Error> {
        let bytes = &migration_level.to_be_bytes();
        let mut txn = self.env.begin_rw_txn()?;
        txn.put(
            self.general,
            b"migration_level",
            &bytes,
            WriteFlags::empty(),
        )?;
        txn.commit()?;
        Ok(())
    }

    pub fn read_migration_level(&self) -> Result<Option<u32>, Error> {
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.general, b"migration_level") {
            Ok(bytes) => Ok(Some(u32::from_be_bytes(bytes[..4].try_into()?))),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn write_encrypted_private_key(
        &self,
        epk: &Option<EncryptedPrivateKey>,
    ) -> Result<(), Error> {
        let bytes = epk.as_ref().map(|e| &e.0).write_to_vec()?;
        let mut txn = self.env.begin_rw_txn()?;
        txn.put(
            self.general,
            b"encrypted_private_key",
            &bytes,
            WriteFlags::empty(),
        )?;
        txn.commit()?;
        Ok(())
    }

    pub fn read_encrypted_private_key(&self) -> Result<Option<EncryptedPrivateKey>, Error> {
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.general, b"encrypted_private_key") {
            Ok(bytes) => {
                let os = Option::<String>::read_from_buffer(bytes)?;
                Ok(os.map(EncryptedPrivateKey))
            }
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn write_last_contact_list_edit(&self, when: i64) -> Result<(), Error> {
        let bytes = &when.to_be_bytes();
        let mut txn = self.env.begin_rw_txn()?;
        txn.put(
            self.general,
            b"last_contact_list_edit",
            &bytes,
            WriteFlags::empty(),
        )?;
        txn.commit()?;
        Ok(())
    }

    pub fn read_last_contact_list_edit(&self) -> Result<Option<i64>, Error> {
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.general, b"last_contact_list_edit") {
            Ok(bytes) => Ok(Some(i64::from_be_bytes(bytes[..8].try_into()?))),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn write_settings(&self, settings: &Settings) -> Result<(), Error> {
        let bytes = settings.write_to_vec()?;
        let mut txn = self.env.begin_rw_txn()?;
        txn.put(self.general, b"settings", &bytes, WriteFlags::empty())?;
        txn.commit()?;
        Ok(())
    }

    pub fn read_settings(&self) -> Result<Option<Settings>, Error> {
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.general, b"settings") {
            Ok(bytes) => Ok(Some(Settings::read_from_buffer(bytes)?)),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
