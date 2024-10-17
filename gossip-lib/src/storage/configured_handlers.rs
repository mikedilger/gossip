use super::types::{ByteRep, HandlerKey};
use crate::error::Error;
use crate::storage::{RawDatabase, Storage};
use heed::types::Bytes;
use heed::RwTxn;
use nostr_types::EventKind;
use std::sync::Mutex;

// (EventKind, HandlerKey) -> u8(flags)

const ENABLED: u8 = 0x1;
const RECOMMENDED: u8 = 0x2;

fn configured_handlers_key_to_bytes(kind: EventKind, hk: HandlerKey) -> Result<Vec<u8>, Error> {
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend(u32::from(kind).to_be_bytes());
    bytes.extend(hk.to_bytes()?);
    Ok(bytes)
}

fn configured_handlers_bytes_to_key(bytes: &[u8]) -> Result<(EventKind, HandlerKey), Error> {
    let key = u32::from_be_bytes(bytes[0..4].try_into().unwrap()).into();
    let hk = HandlerKey::from_bytes(&bytes[4..])?;
    Ok((key, hk))
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
                    .name("configured_handlers_redux") // redux because was used on unstable
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
        recommended: bool,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        let mut flags: u8 = 0;
        if enabled {
            flags |= ENABLED;
        }
        if recommended {
            flags |= RECOMMENDED;
        }

        let key = configured_handlers_key_to_bytes(kind, handler_key)?;
        let val = vec![flags];
        self.db_configured_handlers()?.put(txn, &key, &val)?;

        maybe_local_txn_commit!(local_txn);
        Ok(())
    }

    /// Read a configured handler (key, enabled, recommended)
    pub fn read_configured_handlers(
        &self,
        kind: EventKind,
    ) -> Result<Vec<(HandlerKey, bool, bool)>, Error> {
        let txn = self.get_read_txn()?;

        let mut output: Vec<(HandlerKey, bool, bool)> = Vec::new();
        let prefix: Vec<u8> = u32::from(kind).to_be_bytes().into();
        let iter = self.db_configured_handlers()?.prefix_iter(&txn, &prefix)?;
        for result in iter {
            let (key, val) = result?;
            let (_kind, handler_key) = configured_handlers_bytes_to_key(key)?;
            let (enabled, recommended) = if val.len() == 0 {
                (false, false)
            } else {
                (val[0] & ENABLED != 0, val[0] & RECOMMENDED != 0)
            };
            output.push((handler_key, enabled, recommended));
        }

        Ok(output)
    }

    pub fn read_all_configured_handlers(
        &self,
    ) -> Result<Vec<(EventKind, HandlerKey, bool, bool)>, Error> {
        let txn = self.env.read_txn()?;
        let mut output: Vec<(EventKind, HandlerKey, bool, bool)> = Vec::new();
        let iter = self.db_configured_handlers()?.iter(&txn)?;
        for result in iter {
            let (key, val) = result?;
            let (kind, handler_key) = configured_handlers_bytes_to_key(key)?;
            let (enabled, recommended) = if val.len() == 0 {
                (false, false)
            } else {
                (val[0] & ENABLED != 0, val[0] & RECOMMENDED != 0)
            };
            output.push((kind, handler_key, enabled, recommended));
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
