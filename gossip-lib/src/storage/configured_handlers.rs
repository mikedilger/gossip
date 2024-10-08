use super::types::{ByteRep, HandlerKey};
use crate::error::Error;
use crate::storage::{RawDatabase, Storage};
use heed::types::Bytes;
use heed::{DatabaseFlags, RwTxn};
use nostr_types::EventKind;
use std::sync::Mutex;

// EventKind -> (HandlerKey, bool)

pub fn configured_handlers_key_to_bytes(kind: EventKind) -> Vec<u8> {
    u32::from(kind).to_be_bytes().into()
}

//pub fn configured_handlers_bytes_to_key(bytes: &[u8]) -> EventKind {
//    u32::from_be_bytes(bytes[0..4].try_into().unwrap()).into()
//}

pub fn configured_handlers_val_to_bytes(hk: HandlerKey, enabled: bool) -> Result<Vec<u8>, Error> {
    let mut bytes: Vec<u8> = hk.to_bytes()?;
    bytes.push(if enabled { 1_u8 } else { 0_u8 });
    Ok(bytes)
}

pub fn configured_handlers_bytes_to_val(bytes: &[u8]) -> Result<(HandlerKey, bool), Error> {
    let hk = HandlerKey::from_bytes(&bytes[..bytes.len() - 1])?;
    let enabled: bool = bytes[bytes.len() - 1] != 0;
    Ok((hk, enabled))
}

static CONFIGURED_HANDLERS_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut CONFIGURED_HANDLERS_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_configured_handlers(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = CONFIGURED_HANDLERS_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = CONFIGURED_HANDLERS_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = CONFIGURED_HANDLERS_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env.write_txn()?;
                let db = self
                    .env
                    .database_options()
                    .types::<Bytes, Bytes>()
                    .flags(DatabaseFlags::DUP_SORT) // not DUP_FIXED as HandlerKey isn't fixed size
                    .name("configured_handlers")
                    .create(&mut txn)?;
                txn.commit()?;
                CONFIGURED_HANDLERS_DB = Some(db);
                Ok(db)
            }
        }
    }

    /// Write a configured handler
    pub fn write_configured_handler<'a>(
        &'a self,
        kind: EventKind,
        handler_key: HandlerKey,
        enabled: bool,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        self.db_configured_handlers()?.put(
            txn,
            &configured_handlers_key_to_bytes(kind),
            &configured_handlers_val_to_bytes(handler_key, enabled)?,
        )?;

        maybe_local_txn_commit!(local_txn);
        Ok(())
    }

    /// Read a configured handler
    pub fn read_configured_handlers(
        &self,
        kind: EventKind,
    ) -> Result<Vec<(HandlerKey, bool)>, Error> {
        let txn = self.get_read_txn()?;

        let key = configured_handlers_key_to_bytes(kind);

        let mut output: Vec<(HandlerKey, bool)> = Vec::new();

        let iter = match self.db_configured_handlers()?.get_duplicates(&txn, &key)? {
            Some(i) => i,
            None => return Ok(vec![]),
        };
        for result in iter {
            let (_key, val) = result?;
            let handler: (HandlerKey, bool) = configured_handlers_bytes_to_val(val)?;
            output.push(handler);
        }

        Ok(output)
    }

    pub fn clear_configured_handlers<'a>(
        &'a self,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        self.db_configured_handlers()?.clear(txn)?;

        maybe_local_txn_commit!(local_txn);
        Ok(())
    }
}
