use crate::error::Error;
use crate::storage::types::PersonRelay2;
use crate::storage::{RawDatabase, Storage, MAX_LMDB_KEY};
use heed::types::Bytes;
use heed::RwTxn;
use nostr_types::{PublicKey, RelayUrl};
use speedy::{Readable, Writable};
use std::sync::Mutex;

// PublicKey:Url -> PersonRelay2
//   key: key!(pubkey.as_bytes + url.as_str().as_bytes)
//   val: person_relay.write_to_vec) | PersonRelay::read_from_buffer(bytes)

static PERSON_RELAYS2_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut PERSON_RELAYS2_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_person_relays2(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = PERSON_RELAYS2_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = PERSON_RELAYS2_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = PERSON_RELAYS2_DB {
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
                    .name("person_relays2")
                    .create(&mut txn)?;
                txn.commit()?;
                PERSON_RELAYS2_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub(crate) fn get_person_relays2_size(&self) -> Result<usize, Error> {
        let txn = self.env.read_txn()?;
        let stat = self.db_person_relays2()?.stat(&txn)?;
        Ok(stat.page_size as usize
            * (stat.branch_pages + stat.leaf_pages + stat.overflow_pages + 2) as usize)
    }

    #[allow(dead_code)]
    pub(crate) fn write_person_relay2<'a>(
        &'a self,
        person_relay: &PersonRelay2,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut key = person_relay.pubkey.to_bytes();
        key.extend(person_relay.url.as_str().as_bytes());
        key.truncate(MAX_LMDB_KEY);
        let bytes = person_relay.write_to_vec()?;

        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        self.db_person_relays2()?.put(txn, &key, &bytes)?;

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }

    pub(crate) fn read_person_relay2(
        &self,
        pubkey: PublicKey,
        url: &RelayUrl,
    ) -> Result<Option<PersonRelay2>, Error> {
        let mut key = pubkey.to_bytes();
        key.extend(url.as_str().as_bytes());
        key.truncate(MAX_LMDB_KEY);
        let txn = self.env.read_txn()?;
        Ok(match self.db_person_relays2()?.get(&txn, &key)? {
            Some(bytes) => Some(PersonRelay2::read_from_buffer(bytes)?),
            None => None,
        })
    }

    pub(crate) fn get_person_relays2(&self, pubkey: PublicKey) -> Result<Vec<PersonRelay2>, Error> {
        let start_key = pubkey.to_bytes();
        let txn = self.env.read_txn()?;
        let iter = self.db_person_relays2()?.prefix_iter(&txn, &start_key)?;
        let mut output: Vec<PersonRelay2> = Vec::new();
        for result in iter {
            let (_key, val) = result?;
            let person_relay = PersonRelay2::read_from_buffer(val)?;
            output.push(person_relay);
        }
        Ok(output)
    }

    pub(crate) fn have_persons_relays2(&self, pubkey: PublicKey) -> Result<bool, Error> {
        let start_key = pubkey.to_bytes();
        let txn = self.env.read_txn()?;
        let iter = self.db_person_relays2()?.prefix_iter(&txn, &start_key)?;
        for result in iter {
            let (_key, val) = result?;
            let person_relay = PersonRelay2::read_from_buffer(val)?;
            if person_relay.write || person_relay.read {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(crate) fn delete_person_relays2<'a, F>(
        &'a self,
        filter: F,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        F: Fn(&PersonRelay2) -> bool,
    {
        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        // Delete any person_relay with this relay
        let mut deletions: Vec<Vec<u8>> = Vec::new();
        {
            for result in self.db_person_relays2()?.iter(txn)? {
                let (key, val) = result?;
                if let Ok(person_relay) = PersonRelay2::read_from_buffer(val) {
                    if filter(&person_relay) {
                        deletions.push(key.to_owned());
                    }
                }
            }
        }
        for deletion in deletions.drain(..) {
            self.db_person_relays2()?.delete(txn, &deletion)?;
        }

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }

    pub(crate) fn modify_person_relay2<'a, M>(
        &'a self,
        pubkey: PublicKey,
        url: &RelayUrl,
        mut modify: M,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut PersonRelay2),
    {
        let key = {
            let mut key = pubkey.to_bytes();
            key.extend(url.as_str().as_bytes());
            key.truncate(MAX_LMDB_KEY);
            key
        };

        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        let bytes = self.db_person_relays2()?.get(txn, &key)?;
        let mut person_relay = match bytes {
            Some(bytes) => PersonRelay2::read_from_buffer(bytes)?,
            None => PersonRelay2::new(pubkey, url.to_owned()),
        };
        modify(&mut person_relay);
        let bytes = person_relay.write_to_vec()?;
        self.db_person_relays2()?.put(txn, &key, &bytes)?;

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }

    pub(crate) fn modify_all_persons_relays2<'a, M>(
        &'a self,
        pubkey: PublicKey,
        mut modify: M,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut PersonRelay2),
    {
        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        {
            let prefix = pubkey.to_bytes();
            let mut iter = self.db_person_relays2()?.prefix_iter_mut(txn, &prefix)?;
            while let Some(result) = iter.next() {
                let (key, val) = result?;
                let mut person_relay = PersonRelay2::read_from_buffer(val)?;
                modify(&mut person_relay);
                let bytes = person_relay.write_to_vec()?;
                let key = key.to_owned();
                unsafe {
                    iter.put_current(&key, &bytes)?;
                }
            }
        }

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }
}
