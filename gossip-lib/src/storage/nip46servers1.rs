use std::sync::Mutex;

use heed::types::UnalignedSlice;
use heed::RwTxn;
use speedy::Writable;

use crate::error::Error;
use crate::nip46::Nip46Server;
use crate::storage::{RawDatabase, Storage};

// PublicKey -> Nip46Server
//   key: pubkey.as_bytes()
//   val: nip46server.write_to_vec() | Nip46Server::read_from_buffer(val)

static NIP46SERVER1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut NIP46SERVER1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_nip46servers1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = NIP46SERVER1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = NIP46SERVER1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = NIP46SERVER1_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env.write_txn()?;
                let db = self
                    .env
                    .database_options()
                    .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
                    // no .flags needed
                    .name("nip46server1")
                    .create(&mut txn)?;
                txn.commit()?;
                NIP46SERVER1_DB = Some(db);
                Ok(db)
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn write_nip46server1<'a>(
        &'a self,
        server: &Nip46Server,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let key = server.peer_pubkey.as_bytes();
        let bytes = server.write_to_vec()?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_nip46servers1()?.put(txn, key, &bytes)?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }
}
