use crate::error::{Error, ErrorKind};
use crate::storage::types::Relay2;
use crate::storage::{RawDatabase, Storage};
use heed::types::UnalignedSlice;
use heed::{RoTxn, RwTxn};
use nostr_types::RelayUrl;
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
                    .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
                    // no .flags needed
                    .name("relays2")
                    .create(&mut txn)?;
                txn.commit()?;
                RELAYS2_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub(crate) fn get_relays2_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.db_relays2()?.len(&txn)?)
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

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_relays2()?.put(txn, key, &bytes)?;
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

    pub(crate) fn delete_relay2<'a>(
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
            self.db_relays2()?.delete(txn, key)?;
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

    pub(crate) fn modify_relay2<'a, M>(
        &'a self,
        url: &RelayUrl,
        mut modify: M,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut Relay2),
    {
        let key = key!(url.as_str().as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("relay url".to_owned()).into());
        }

        let mut f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let bytes = self.db_relays2()?.get(txn, key)?;
            let mut relay = match bytes {
                Some(bytes) => serde_json::from_slice(bytes)?,
                None => Relay2::new(url.to_owned()),
            };
            modify(&mut relay);
            let bytes = serde_json::to_vec(&relay)?;
            self.db_relays2()?.put(txn, key, &bytes)?;
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

    pub(crate) fn modify_all_relays2<'a, M>(
        &'a self,
        mut modify: M,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut Relay2),
    {
        let mut f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let mut iter = self.db_relays2()?.iter_mut(txn)?;
            while let Some(result) = iter.next() {
                let (key, val) = result?;
                let mut dbrelay: Relay2 = serde_json::from_slice(val)?;
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

    pub(crate) fn read_relay2<'a>(
        &'a self,
        url: &RelayUrl,
        txn: Option<&RoTxn<'a>>,
    ) -> Result<Option<Relay2>, Error> {
        let f = |txn: &RoTxn<'a>| -> Result<Option<Relay2>, Error> {
            // Note that we use serde instead of speedy because the complexity of the
            // serde_json::Value type makes it difficult. Any other serde serialization
            // should work though: Consider bincode.
            let key = key!(url.as_str().as_bytes());
            if key.is_empty() {
                return Err(ErrorKind::Empty("relay url".to_owned()).into());
            }
            match self.db_relays2()?.get(txn, key)? {
                Some(bytes) => Ok(Some(serde_json::from_slice(bytes)?)),
                None => Ok(None),
            }
        };

        match txn {
            Some(txn) => f(txn),
            None => {
                let txn = self.env.read_txn()?;
                f(&txn)
            }
        }
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
