use crate::error::{Error, ErrorKind};
use crate::storage::types::Relay3;
use crate::storage::{RawDatabase, Storage};
use heed::types::Bytes;
use heed::RwTxn;
use nostr_types::RelayUrl;
use std::sync::Mutex;

// Url -> Relay
//   key: key!(url.0.as_bytes())
//   val: serde_json::to_vec(relay) | serde_json::from_slice(bytes)

static RELAYS3_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut RELAYS3_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_relays3(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = RELAYS3_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = RELAYS3_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = RELAYS3_DB {
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
                    .name("relays3")
                    .create(&mut txn)?;
                txn.commit()?;
                RELAYS3_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub(crate) fn get_relays3_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.db_relays3()?.len(&txn)?)
    }

    #[allow(dead_code)]
    pub(crate) fn write_relay3<'a>(
        &'a self,
        relay: &Relay3,
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

        self.db_relays3()?.put(txn, key, &bytes)?;

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }

    pub(crate) fn delete_relay3<'a>(
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

        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        // Delete any PersonRelay with this url
        self.delete_person_relays(|f| f.url == *url, Some(txn))?;

        // Delete the relay
        self.db_relays3()?.delete(txn, key)?;

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }

    pub(crate) fn modify_relay3<'a, M>(
        &'a self,
        url: &RelayUrl,
        mut modify: M,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut Relay3),
    {
        let key = key!(url.as_str().as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("relay url".to_owned()).into());
        }

        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        let bytes = self.db_relays3()?.get(txn, key)?;
        let mut relay = match bytes {
            Some(bytes) => serde_json::from_slice(bytes)?,
            None => Relay3::new(url.to_owned()),
        };
        modify(&mut relay);
        let bytes = serde_json::to_vec(&relay)?;
        self.db_relays3()?.put(txn, key, &bytes)?;

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }

    pub(crate) fn modify_all_relays3<'a, M>(
        &'a self,
        mut modify: M,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut Relay3),
    {
        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        {
            let mut iter = self.db_relays3()?.iter_mut(txn)?;
            while let Some(result) = iter.next() {
                let (key, val) = result?;
                let mut dbrelay: Relay3 = serde_json::from_slice(val)?;
                modify(&mut dbrelay);
                let bytes = serde_json::to_vec(&dbrelay)?;
                // to deal with the unsafety of put_current
                let key = key.to_owned();
                unsafe {
                    iter.put_current(&key, &bytes)?;
                }
            }
        }

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }

    pub(crate) fn read_relay3(&self, url: &RelayUrl) -> Result<Option<Relay3>, Error> {
        let txn = self.get_read_txn()?;

        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key = key!(url.as_str().as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("relay url".to_owned()).into());
        }
        match self.db_relays3()?.get(&txn, key)? {
            Some(bytes) => Ok(Some(serde_json::from_slice(bytes)?)),
            None => Ok(None),
        }
    }

    pub(crate) fn filter_relays3<F>(&self, f: F) -> Result<Vec<Relay3>, Error>
    where
        F: Fn(&Relay3) -> bool,
    {
        let txn = self.env.read_txn()?;
        let mut output: Vec<Relay3> = Vec::new();
        let iter = self.db_relays3()?.iter(&txn)?;
        for result in iter {
            let (_key, val) = result?;
            let relay: Relay3 = serde_json::from_slice(val)?;
            if f(&relay) {
                output.push(relay);
            }
        }
        Ok(output)
    }
}
