use crate::error::Error;
use crate::nip46::Nip46Server;
use crate::storage::{RawDatabase, Storage};
use heed::types::Bytes;
use heed::RwTxn;
use nostr_types::PublicKey;
use speedy::{Readable, Writable};
use std::sync::Mutex;

// PublicKey -> Nip46Server
//   key: pubkey.as_bytes()
//   val: nip46server.write_to_vec() | Nip46Server::read_from_buffer(val)

static NIP46SERVER2_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut NIP46SERVER2_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_nip46servers2(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = NIP46SERVER2_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = NIP46SERVER2_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = NIP46SERVER2_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env.write_txn()?;
                let db = self
                    .env
                    .database_options()
                    .types::<Bytes, Bytes>()
                    // no .flags needed
                    .name("nip46server2")
                    .create(&mut txn)?;
                txn.commit()?;
                NIP46SERVER2_DB = Some(db);
                Ok(db)
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn write_nip46server2<'a>(
        &'a self,
        server: &Nip46Server,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let key = server.peer_pubkey.as_bytes();
        let bytes = server.write_to_vec()?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_nip46servers2()?.put(txn, key, &bytes)?;
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

    pub(crate) fn read_nip46server2(
        &self,
        pubkey: PublicKey,
    ) -> Result<Option<Nip46Server>, Error> {
        let key = pubkey.as_bytes();
        let txn = self.env.read_txn()?;
        Ok(match self.db_nip46servers2()?.get(&txn, key)? {
            Some(bytes) => Some(Nip46Server::read_from_buffer(bytes)?),
            None => None,
        })
    }

    pub(crate) fn read_all_nip46servers2(&self) -> Result<Vec<Nip46Server>, Error> {
        let txn = self.env.read_txn()?;
        let mut output: Vec<Nip46Server> = Vec::new();
        for result in self.db_nip46servers2()?.iter(&txn)? {
            let (_key, val) = result?;
            let server = Nip46Server::read_from_buffer(val)?;
            output.push(server);
        }
        Ok(output)
    }

    pub(crate) fn delete_nip46server2<'a>(
        &'a self,
        pubkey: PublicKey,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let key = pubkey.as_bytes();

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let _ = self.db_nip46servers2()?.delete(txn, key);
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
