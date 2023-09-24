use crate::error::Error;
use crate::people::PersonList;
use crate::storage::{RawDatabase, Storage};
use heed::types::UnalignedSlice;
use heed::RwTxn;
use nostr_types::PublicKey;
use std::sync::Mutex;

// Pubkey -> Vec<u8>
//   key: pubkey.as_bytes()

static PERSON_LISTS1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut PERSON_LISTS1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_person_lists1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = PERSON_LISTS1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = PERSON_LISTS1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = PERSON_LISTS1_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env.write_txn()?;
                let db = self
                    .env
                    .database_options()
                    .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
                    .name("person_lists")
                    .create(&mut txn)?;
                txn.commit()?;
                PERSON_LISTS1_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub fn read_person_lists1(&self, pubkey: &PublicKey) -> Result<Vec<PersonList>, Error> {
        let key: Vec<u8> = pubkey.to_bytes();
        let txn = self.env.read_txn()?;
        Ok(match self.db_person_lists1()?.get(&txn, &key)? {
            Some(bytes) => bytes.iter().map(|u| (*u).into()).collect(),
            None => vec![],
        })
    }

    pub fn write_person_lists1<'a>(
        &'a self,
        pubkey: &PublicKey,
        lists: Vec<PersonList>,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let key: Vec<u8> = pubkey.to_bytes();
        let bytes = lists.iter().map(|l| (*l).into()).collect::<Vec<u8>>();

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_person_lists1()?.put(txn, &key, &bytes)?;
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

    pub fn get_people_in_list1(&self, list: PersonList) -> Result<Vec<PublicKey>, Error> {
        let txn = self.env.read_txn()?;
        let mut pubkeys: Vec<PublicKey> = Vec::new();
        for result in self.db_person_lists1()?.iter(&txn)? {
            let (key, val) = result?;
            let pubkey = PublicKey::from_bytes(key, true)?;
            let person_lists = val.iter().map(|u| (*u).into()).collect::<Vec<PersonList>>();
            if person_lists.iter().any(|s| *s == list) {
                pubkeys.push(pubkey);
            }
        }
        Ok(pubkeys)
    }

    pub fn clear_person_list1<'a>(
        &'a self,
        list: PersonList,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let mut fixed: Vec<(PublicKey, Vec<u8>)> = Vec::new();

            // Collect records that require changing
            for result in self.db_person_lists1()?.iter(txn)? {
                let (key, val) = result?;
                let pubkey = PublicKey::from_bytes(key, true)?;
                let mut person_lists = val.iter().map(|u| (*u).into()).collect::<Vec<PersonList>>();
                if person_lists.contains(&list) {
                    person_lists = person_lists.drain(..).filter(|l| *l != list).collect();
                    let bytes = person_lists
                        .iter()
                        .map(|l| (*l).into())
                        .collect::<Vec<u8>>();
                    fixed.push((pubkey, bytes));
                }
            }

            // Change them
            for (pubkey, bytes) in fixed.drain(..) {
                let key: Vec<u8> = pubkey.to_bytes();
                self.db_person_lists1()?.put(txn, &key, &bytes)?;
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
}
