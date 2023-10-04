use crate::error::Error;
use crate::storage::types::PersonRelay1;
use crate::storage::{RawDatabase, Storage, MAX_LMDB_KEY};
use heed::types::UnalignedSlice;
use heed::RwTxn;
use nostr_types::{PublicKey, RelayUrl};
use speedy::{Readable, Writable};
use std::sync::Mutex;

// PublicKey:Url -> PersonRelay
//   key: key!(pubkey.as_bytes + url.as_str().as_bytes)
//   val: person_relay.write_to_vec) | PersonRelay::read_from_buffer(bytes)

static PERSON_RELAYS1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut PERSON_RELAYS1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_person_relays1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = PERSON_RELAYS1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = PERSON_RELAYS1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = PERSON_RELAYS1_DB {
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
                    .name("person_relays")
                    .create(&mut txn)?;
                txn.commit()?;
                PERSON_RELAYS1_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub fn get_person_relays1_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.db_person_relays1()?.len(&txn)?)
    }

    #[allow(dead_code)]
    pub fn write_person_relay1<'a>(
        &'a self,
        person_relay: &PersonRelay1,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut key = person_relay.pubkey.to_bytes();
        key.extend(person_relay.url.as_str().as_bytes());
        key.truncate(MAX_LMDB_KEY);
        let bytes = person_relay.write_to_vec()?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_person_relays1()?.put(txn, &key, &bytes)?;
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

    pub fn read_person_relay1(
        &self,
        pubkey: PublicKey,
        url: &RelayUrl,
    ) -> Result<Option<PersonRelay1>, Error> {
        let mut key = pubkey.to_bytes();
        key.extend(url.as_str().as_bytes());
        key.truncate(MAX_LMDB_KEY);
        let txn = self.env.read_txn()?;
        Ok(match self.db_person_relays1()?.get(&txn, &key)? {
            Some(bytes) => Some(PersonRelay1::read_from_buffer(bytes)?),
            None => None,
        })
    }

    pub fn get_person_relays1(&self, pubkey: PublicKey) -> Result<Vec<PersonRelay1>, Error> {
        let start_key = pubkey.to_bytes();
        let txn = self.env.read_txn()?;
        let iter = self.db_person_relays1()?.prefix_iter(&txn, &start_key)?;
        let mut output: Vec<PersonRelay1> = Vec::new();
        for result in iter {
            let (_key, val) = result?;
            let person_relay = PersonRelay1::read_from_buffer(val)?;
            output.push(person_relay);
        }
        Ok(output)
    }

    pub fn have_persons_relays1(&self, pubkey: PublicKey) -> Result<bool, Error> {
        let start_key = pubkey.to_bytes();
        let txn = self.env.read_txn()?;
        let iter = self.db_person_relays1()?.prefix_iter(&txn, &start_key)?;
        for result in iter {
            let (_key, val) = result?;
            let person_relay = PersonRelay1::read_from_buffer(val)?;
            if person_relay.write
                || person_relay.read
                || person_relay.manually_paired_read
                || person_relay.manually_paired_write
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn delete_person_relays1<'a, F>(
        &'a self,
        filter: F,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        F: Fn(&PersonRelay1) -> bool,
    {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            // Delete any person_relay with this relay
            let mut deletions: Vec<Vec<u8>> = Vec::new();
            {
                for result in self.db_person_relays1()?.iter(txn)? {
                    let (key, val) = result?;
                    if let Ok(person_relay) = PersonRelay1::read_from_buffer(val) {
                        if filter(&person_relay) {
                            deletions.push(key.to_owned());
                        }
                    }
                }
            }
            for deletion in deletions.drain(..) {
                self.db_person_relays1()?.delete(txn, &deletion)?;
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
