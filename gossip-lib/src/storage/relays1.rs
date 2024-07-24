use crate::error::{Error, ErrorKind};
use crate::storage::types::Relay1;
use crate::storage::{RawDatabase, Storage};
use heed::types::Bytes;
use heed::RwTxn;
use nostr_types::RelayUrl;
use std::sync::Mutex;

// Url -> Relay
//   key: key!(url.0.as_bytes())
//   val: serde_json::to_vec(relay) | serde_json::from_slice(bytes)

static RELAYS1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut RELAYS1_DB: Option<RawDatabase> = None;

impl Storage {
    #[allow(dead_code)]
    pub(super) fn db_relays1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = RELAYS1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = RELAYS1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = RELAYS1_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env().write_txn()?;
                let db = self
                    .env()
                    .database_options()
                    .types::<Bytes, Bytes>()
                    // no .flags needed
                    .name("relays")
                    .create(&mut txn)?;
                txn.commit()?;
                RELAYS1_DB = Some(db);
                Ok(db)
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn get_relays1_len(&self) -> Result<u64, Error> {
        let txn = self.env().read_txn()?;
        Ok(self.db_relays1()?.len(&txn)?)
    }

    #[allow(dead_code)]
    pub(crate) fn write_relay1<'a>(
        &'a self,
        relay: &Relay1,
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

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_relays1()?.put(txn, key, &bytes)?;
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    #[allow(dead_code)]
    pub(crate) fn delete_relay1<'a>(
        &'a self,
        url: &RelayUrl,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key = key!(url.as_str().as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("relay url".to_owned()).into());
        }

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            // Delete any PersonRelay with this url
            self.delete_person_relays(|f| f.url == *url, Some(txn))?;

            // Delete the relay
            self.db_relays1()?.delete(txn, key)?;
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    #[allow(dead_code)]
    pub(crate) fn modify_relay1<'a, M>(
        &'a self,
        url: &RelayUrl,
        mut modify: M,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut Relay1),
    {
        let key = key!(url.as_str().as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("relay url".to_owned()).into());
        }

        let mut f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let bytes = self.db_relays1()?.get(txn, key)?;
            let mut relay = match bytes {
                Some(bytes) => serde_json::from_slice(bytes)?,
                None => Relay1::new(url.to_owned()),
            };
            modify(&mut relay);
            let bytes = serde_json::to_vec(&relay)?;
            self.db_relays1()?.put(txn, key, &bytes)?;
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    #[allow(dead_code)]
    pub(crate) fn modify_all_relays1<'a, M>(
        &'a self,
        mut modify: M,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut Relay1),
    {
        let mut f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let mut iter = self.db_relays1()?.iter_mut(txn)?;
            while let Some(result) = iter.next() {
                let (key, val) = result?;
                let mut dbrelay: Relay1 = serde_json::from_slice(val)?;
                modify(&mut dbrelay);
                let bytes = serde_json::to_vec(&dbrelay)?;
                // to deal with the unsafety of put_current
                let key = key.to_owned();
                unsafe {
                    iter.put_current(&key, &bytes)?;
                }
            }
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    #[allow(dead_code)]
    pub(crate) fn read_relay1(&self, url: &RelayUrl) -> Result<Option<Relay1>, Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key = key!(url.as_str().as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("relay url".to_owned()).into());
        }
        let txn = self.env().read_txn()?;
        match self.db_relays1()?.get(&txn, key)? {
            Some(bytes) => Ok(Some(serde_json::from_slice(bytes)?)),
            None => Ok(None),
        }
    }

    pub(crate) fn filter_relays1<F>(&self, f: F) -> Result<Vec<Relay1>, Error>
    where
        F: Fn(&Relay1) -> bool,
    {
        let txn = self.env().read_txn()?;
        let mut output: Vec<Relay1> = Vec::new();
        let iter = self.db_relays1()?.iter(&txn)?;
        for result in iter {
            let (_key, val) = result?;
            let relay: Relay1 = serde_json::from_slice(val)?;
            if f(&relay) {
                output.push(relay);
            }
        }
        Ok(output)
    }
}
