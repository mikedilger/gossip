use crate::error::Error;
use crate::storage::types::PersonRelay1;
use crate::storage::{RawDatabase, Storage, MAX_LMDB_KEY};
use heed::types::Bytes;
use heed::RwTxn;
use speedy::Writable;
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
                    .types::<Bytes, Bytes>()
                    // no .flags needed
                    .name("person_relays")
                    .create(&mut txn)?;
                txn.commit()?;
                PERSON_RELAYS1_DB = Some(db);
                Ok(db)
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn write_person_relay1<'a>(
        &'a self,
        person_relay: &PersonRelay1,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut key = person_relay.pubkey.to_bytes();
        key.extend(person_relay.url.as_str().as_bytes());
        key.truncate(MAX_LMDB_KEY);
        let bytes = person_relay.write_to_vec()?;

        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        self.db_person_relays1()?.put(txn, &key, &bytes)?;

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }
}
