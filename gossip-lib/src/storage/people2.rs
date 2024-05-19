use crate::error::Error;
use crate::storage::types::Person2;
use crate::storage::{RawDatabase, Storage};
use heed::types::Bytes;
use heed::{RoTxn, RwTxn};
use nostr_types::PublicKey;
use std::sync::Mutex;

// PublicKey -> Person
//   key: pubkey.as_bytes()
//   val: serde_json::to_vec(person) | serde_json::from_slice(bytes)

static PEOPLE2_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut PEOPLE2_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_people2(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = PEOPLE2_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = PEOPLE2_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = PEOPLE2_DB {
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
                    .name("people2")
                    .create(&mut txn)?;
                txn.commit()?;
                PEOPLE2_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub(crate) fn get_people2_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.db_people2()?.len(&txn)?)
    }

    #[allow(dead_code)]
    pub(crate) fn write_person2<'a>(
        &'a self,
        person: &Person2,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key: Vec<u8> = person.pubkey.to_bytes();
        let bytes = serde_json::to_vec(person)?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_people2()?.put(txn, &key, &bytes)?;
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    pub(crate) fn has_person2<'a>(
        &'a self,
        pubkey: &PublicKey,
        txn: Option<&RoTxn<'a>>,
    ) -> Result<bool, Error> {
        let f = |txn: &RoTxn<'a>| -> Result<bool, Error> {
            let key: Vec<u8> = pubkey.to_bytes();
            Ok(self.db_people2()?.get(txn, &key)?.is_some())
        };

        read_transact!(self, txn, f)
    }

    pub(crate) fn read_person2<'a>(
        &'a self,
        pubkey: &PublicKey,
        txn: Option<&RoTxn<'a>>,
    ) -> Result<Option<Person2>, Error> {
        let f = |txn: &RoTxn<'a>| -> Result<Option<Person2>, Error> {
            // Note that we use serde instead of speedy because the complexity of the
            // serde_json::Value type makes it difficult. Any other serde serialization
            // should work though: Consider bincode.
            let key: Vec<u8> = pubkey.to_bytes();
            Ok(match self.db_people2()?.get(txn, &key)? {
                Some(bytes) => Some(serde_json::from_slice(bytes)?),
                None => None,
            })
        };

        read_transact!(self, txn, f)
    }

    pub(crate) fn filter_people2<F>(&self, f: F) -> Result<Vec<Person2>, Error>
    where
        F: Fn(&Person2) -> bool,
    {
        let txn = self.env.read_txn()?;
        let iter = self.db_people2()?.iter(&txn)?;
        let mut output: Vec<Person2> = Vec::new();
        for result in iter {
            let (_key, val) = result?;
            let person: Person2 = serde_json::from_slice(val)?;
            if f(&person) {
                output.push(person);
            }
        }
        Ok(output)
    }

    pub(crate) fn modify_person2<'a, M>(
        &'a self,
        pubkey: PublicKey,
        mut modify: M,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut Person2),
    {
        let key = key!(pubkey.as_bytes());

        let mut f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let bytes = self.db_people2()?.get(txn, key)?;
            let mut person = match bytes {
                Some(bytes) => serde_json::from_slice(bytes)?,
                None => Person2::new(pubkey.to_owned()),
            };
            modify(&mut person);
            let bytes = serde_json::to_vec(&person)?;
            self.db_people2()?.put(txn, key, &bytes)?;
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    pub(crate) fn modify_all_people2<'a, M>(
        &'a self,
        mut modify: M,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut Person2),
    {
        let mut f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let mut iter = self.db_people2()?.iter_mut(txn)?;
            while let Some(result) = iter.next() {
                let (key, val) = result?;
                let mut person: Person2 = serde_json::from_slice(val)?;
                modify(&mut person);
                let bytes = serde_json::to_vec(&person)?;
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
}
