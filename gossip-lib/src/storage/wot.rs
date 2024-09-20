use crate::error::Error;
use crate::storage::{RawDatabase, Storage};
use heed::types::Bytes;
use heed::RwTxn;
use nostr_types::PublicKey;
use std::sync::Mutex;

// Pubkey -> u64
//   key: key!(pubkey.as_bytes())
//   val: u64.as_be_bytes();

static WOT_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut WOT_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_wot(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = WOT_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = WOT_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = WOT_DB {
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
                    .name("wot")
                    .create(&mut txn)?;
                txn.commit()?;
                WOT_DB = Some(db);
                Ok(db)
            }
        }
    }

    // Write wot
    #[allow(dead_code)]
    pub(crate) fn write_wot<'a>(
        &'a self,
        pubkey: PublicKey,
        wot: u64,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        self.db_wot()?
            .put(txn, pubkey.as_bytes(), wot.to_be_bytes().as_slice())?;

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }

    // Read wot
    pub fn read_wot<'a>(&'a self, pubkey: PublicKey) -> Result<u64, Error> {
        let txn = self.get_read_txn()?;
        let wot = match self.db_wot()?.get(&txn, pubkey.as_bytes())? {
            Some(bytes) => u64::from_be_bytes(<[u8; 8]>::try_from(&bytes[..8]).unwrap()),
            None => 0,
        };
        Ok(wot)
    }

    // Incr wot
    pub(crate) fn incr_wot<'a>(
        &'a self,
        pubkey: PublicKey,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        let mut wot = match self.db_wot()?.get(txn, pubkey.as_bytes())? {
            Some(bytes) => u64::from_be_bytes(<[u8; 8]>::try_from(&bytes[..8]).unwrap()),
            None => 0,
        };
        wot += 1;
        self.db_wot()?
            .put(txn, pubkey.as_bytes(), wot.to_be_bytes().as_slice())?;

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }

    // Decr wot
    pub(crate) fn decr_wot<'a>(
        &'a self,
        pubkey: PublicKey,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        let mut wot = match self.db_wot()?.get(txn, pubkey.as_bytes())? {
            Some(bytes) => u64::from_be_bytes(<[u8; 8]>::try_from(&bytes[..8]).unwrap()),
            None => 0,
        };
        if wot > 0 {
            wot -= 1;
        }
        self.db_wot()?
            .put(txn, pubkey.as_bytes(), wot.to_be_bytes().as_slice())?;

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }
}
