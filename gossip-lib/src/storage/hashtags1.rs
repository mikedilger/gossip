use crate::error::{Error, ErrorKind};
use crate::storage::{RawDatabase, Storage};
use heed::{types::Bytes, DatabaseFlags, RwTxn};
use nostr_types::Id;
use std::sync::Mutex;

// Hashtag -> Id
// (dup keys, so multiple Ids per hashtag)
//   key: key!(hashtag.as_bytes())
//   val: id.as_slice() | Id(val[0..32].try_into()?)

static HASHTAGS1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut HASHTAGS1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_hashtags1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = HASHTAGS1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = HASHTAGS1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = HASHTAGS1_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env.write_txn()?;
                let db = self
                    .env
                    .database_options()
                    .types::<Bytes, Bytes>()
                    .flags(DatabaseFlags::DUP_SORT | DatabaseFlags::DUP_FIXED)
                    .name("hashtags")
                    .create(&mut txn)?;
                txn.commit()?;
                HASHTAGS1_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub(crate) fn add_hashtag1<'a>(
        &'a self,
        hashtag: &String,
        id: Id,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let key = key!(hashtag.as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("hashtag".to_owned()).into());
        }
        let bytes = id.as_slice();

        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        self.db_hashtags1()?.put(txn, key, bytes)?;

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn get_event_ids_with_hashtag1(&self, hashtag: &String) -> Result<Vec<Id>, Error> {
        let key = key!(hashtag.as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("hashtag".to_owned()).into());
        }
        let txn = self.env.read_txn()?;
        let mut output: Vec<Id> = Vec::new();
        let iter = match self.db_hashtags1()?.get_duplicates(&txn, key)? {
            Some(i) => i,
            None => return Ok(vec![]),
        };
        for result in iter {
            let (_key, val) = result?;
            let id = Id(val[0..32].try_into()?);
            output.push(id);
        }
        Ok(output)
    }
}
