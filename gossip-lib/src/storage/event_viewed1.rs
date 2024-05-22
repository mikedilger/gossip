use crate::error::Error;
use crate::storage::{RawDatabase, Storage};
use heed::types::Bytes;
use heed::RwTxn;
use nostr_types::Id;
use std::sync::Mutex;

// Id -> ()
//   key: id.as_slice()
//   val: vec![]

static EVENT_VIEWED1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut EVENT_VIEWED1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_event_viewed1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = EVENT_VIEWED1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = EVENT_VIEWED1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = EVENT_VIEWED1_DB {
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
                    .name("event_viewed")
                    .create(&mut txn)?;
                txn.commit()?;
                EVENT_VIEWED1_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub(crate) fn get_event_viewed1_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.db_event_viewed1()?.len(&txn)?)
    }

    pub(crate) fn mark_event_viewed1<'a>(
        &'a self,
        id: Id,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let bytes = vec![];

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_event_viewed1()?.put(txn, id.as_slice(), &bytes)?;
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    pub(crate) fn is_event_viewed1(&self, id: Id) -> Result<bool, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.db_event_viewed1()?.get(&txn, id.as_slice())?.is_some())
    }
}
