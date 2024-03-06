use super::types::PersonList1;
use crate::error::Error;
use crate::storage::{RawDatabase, Storage};
use heed::types::UnalignedSlice;
use heed::RwTxn;
use nostr_types::PublicKey;
use speedy::{Readable, Writable};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

// Pubkey -> HashMap<PersonList1, bool> // bool is if private or not
//   key: pubkey.as_bytes()

static PERSON_LISTS2_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut PERSON_LISTS2_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_person_lists2(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = PERSON_LISTS2_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = PERSON_LISTS2_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = PERSON_LISTS2_DB {
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
                    .name("person_lists_2")
                    .create(&mut txn)?;
                txn.commit()?;
                PERSON_LISTS2_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub(crate) fn read_person_lists2(
        &self,
        pubkey: &PublicKey,
    ) -> Result<HashMap<PersonList1, bool>, Error> {
        let key: Vec<u8> = pubkey.to_bytes();
        let txn = self.env.read_txn()?;
        Ok(match self.db_person_lists2()?.get(&txn, &key)? {
            None => HashMap::new(),
            Some(bytes) => HashMap::<PersonList1, bool>::read_from_buffer(bytes)?,
        })
    }

    pub(crate) fn write_person_lists2<'a>(
        &'a self,
        pubkey: &PublicKey,
        map: HashMap<PersonList1, bool>,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let key: Vec<u8> = pubkey.to_bytes();
        let bytes: Vec<u8> = map.write_to_vec()?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_person_lists2()?.put(txn, &key, &bytes)?;
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

    pub(crate) fn get_people_in_all_followed_lists2(&self) -> Result<Vec<PublicKey>, Error> {
        let txn = self.env.read_txn()?;
        let mut pubkeys: Vec<PublicKey> = Vec::new();
        for result in self.db_person_lists2()?.iter(&txn)? {
            let (key, val) = result?;
            let pubkey = PublicKey::from_bytes(key, true)?;
            let map = HashMap::<PersonList1, bool>::read_from_buffer(val)?;
            if map.keys().any(|list| list.subscribe()) {
                pubkeys.push(pubkey);
            }
        }
        Ok(pubkeys)
    }

    pub(crate) fn get_people_in_list2(
        &self,
        list: PersonList1,
    ) -> Result<Vec<(PublicKey, bool)>, Error> {
        let txn = self.env.read_txn()?;
        let mut output: Vec<(PublicKey, bool)> = Vec::new();
        for result in self.db_person_lists2()?.iter(&txn)? {
            let (key, val) = result?;
            let pubkey = PublicKey::from_bytes(key, true)?;
            let map = HashMap::<PersonList1, bool>::read_from_buffer(val)?;
            if let Some(actual_public) = map.get(&list) {
                output.push((pubkey, *actual_public));
            }
        }
        Ok(output)
    }

    pub(crate) fn clear_person_list2<'a>(
        &'a self,
        list: PersonList1,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let mut fixed: Vec<(PublicKey, HashMap<PersonList1, bool>)> = Vec::new();

            // Collect records that require changing
            // (lmdb doesn't like changing them in place while iterating)
            for result in self.db_person_lists2()?.iter(txn)? {
                let (key, val) = result?;
                let pubkey = PublicKey::from_bytes(key, true)?;
                let mut map = HashMap::<PersonList1, bool>::read_from_buffer(val)?;
                if map.contains_key(&list) {
                    map.remove(&list);
                    fixed.push((pubkey, map));
                }
            }

            // Change them
            for (pubkey, map) in fixed.drain(..) {
                let key: Vec<u8> = pubkey.to_bytes();
                if map.is_empty() {
                    self.db_person_lists2()?.delete(txn, &key)?;
                } else {
                    let bytes: Vec<u8> = map.write_to_vec()?;
                    self.db_person_lists2()?.put(txn, &key, &bytes)?;
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

    pub(crate) fn hash_person_list2(&self, list: PersonList1) -> Result<u64, Error> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for (person, private) in self.get_people_in_list2(list)? {
            person.hash(&mut hasher);
            private.hash(&mut hasher);
        }
        Ok(hasher.finish())
    }
}
