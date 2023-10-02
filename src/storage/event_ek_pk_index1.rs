use crate::error::Error;
use crate::storage::{RawDatabase, Storage};
use heed::{DatabaseFlags, types::UnalignedSlice};
use std::sync::Mutex;

// EventKind:PublicKey -> Id
// (pubkey is event author)
// (dup keys, so multiple Ids per key)
//   val: id.as_slice() | Id(val[0..32].try_into()?)

static EVENT_EK_PK_INDEX1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut EVENT_EK_PK_INDEX1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_event_ek_pk_index1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = EVENT_EK_PK_INDEX1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = EVENT_EK_PK_INDEX1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = EVENT_EK_PK_INDEX1_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env.write_txn()?;
                let db = self
                    .env
                    .database_options()
                    .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
                    .flags(DatabaseFlags::DUP_SORT | DatabaseFlags::DUP_FIXED)
                    .name("event_ek_pk_index")
                    .create(&mut txn)?;
                txn.commit()?;
                EVENT_EK_PK_INDEX1_DB = Some(db);
                Ok(db)
            }
        }
    }
}
