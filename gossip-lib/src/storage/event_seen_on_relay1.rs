use crate::error::Error;
use crate::storage::{RawDatabase, Storage, MAX_LMDB_KEY};
use heed::types::UnalignedSlice;
use heed::RwTxn;
use nostr_types::{Id, RelayUrl, Unixtime};
use std::sync::Mutex;

// Id:Url -> Unixtime
//   key: key!(id.as_slice(), url.as_str().as_bytes())
//   val: unixtime.0.to_be_bytes()

static EVENT_SEEN_ON_RELAY1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut EVENT_SEEN_ON_RELAY1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_event_seen_on_relay1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = EVENT_SEEN_ON_RELAY1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = EVENT_SEEN_ON_RELAY1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = EVENT_SEEN_ON_RELAY1_DB {
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
                    .name("event_seen_on_relay")
                    .create(&mut txn)?;
                txn.commit()?;
                EVENT_SEEN_ON_RELAY1_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub fn get_event_seen_on_relay1_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.db_event_seen_on_relay1()?.len(&txn)?)
    }

    pub fn add_event_seen_on_relay1<'a>(
        &'a self,
        id: Id,
        url: &RelayUrl,
        when: Unixtime,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut key: Vec<u8> = id.as_slice().to_owned();
        key.extend(url.as_str().as_bytes());
        key.truncate(MAX_LMDB_KEY);
        let bytes = when.0.to_be_bytes();

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_event_seen_on_relay1()?.put(txn, &key, &bytes)?;
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

    pub fn get_event_seen_on_relay1(&self, id: Id) -> Result<Vec<(RelayUrl, Unixtime)>, Error> {
        let start_key: Vec<u8> = id.as_slice().to_owned();
        let txn = self.env.read_txn()?;
        let mut output: Vec<(RelayUrl, Unixtime)> = Vec::new();
        for result in self
            .db_event_seen_on_relay1()?
            .prefix_iter(&txn, &start_key)?
        {
            let (key, val) = result?;

            // Extract off the Url
            let url = RelayUrl::try_from_str(std::str::from_utf8(&key[32..])?)?;
            let time = Unixtime(i64::from_be_bytes(val[..8].try_into()?));
            output.push((url, time));
        }
        Ok(output)
    }
}
