use crate::error::{Error, ErrorKind};
use crate::storage::types::Relay2;
use crate::storage::{RawDatabase, Storage};
use heed::types::Bytes;
use heed::RwTxn;
use std::sync::Mutex;

// Url -> Relay
//   key: key!(url.0.as_bytes())
//   val: serde_json::to_vec(relay) | serde_json::from_slice(bytes)

static RELAYS2_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut RELAYS2_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_relays2(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = RELAYS2_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = RELAYS2_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = RELAYS2_DB {
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
                    .name("relays2")
                    .create(&mut txn)?;
                txn.commit()?;
                RELAYS2_DB = Some(db);
                Ok(db)
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn write_relay2<'a>(
        &'a self,
        relay: &Relay2,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key = key!(relay.url.as_str().as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("relay url".to_owned()).into());
        }
        let bytes = serde_json::to_vec(relay)?;

        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        self.db_relays2()?.put(txn, key, &bytes)?;

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }

    pub(crate) fn filter_relays2<F>(&self, f: F) -> Result<Vec<Relay2>, Error>
    where
        F: Fn(&Relay2) -> bool,
    {
        let txn = self.env.read_txn()?;
        let mut output: Vec<Relay2> = Vec::new();
        let iter = self.db_relays2()?.iter(&txn)?;
        for result in iter {
            let (_key, val) = result?;
            let relay: Relay2 = serde_json::from_slice(val)?;
            if f(&relay) {
                output.push(relay);
            }
        }
        Ok(output)
    }
}
