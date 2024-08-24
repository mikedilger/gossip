use crate::error::Error;
use crate::storage::{RawDatabase, Storage};
use heed::types::Bytes;
use std::sync::Mutex;

// Id:Id -> RelationshipById1
//   key: id.as_slice(), id.as_slice() | Id(val[32..64].try_into()?)
//   val:  relationship_by_id.write_to_vec() | RelationshipById1::read_from_buffer(val)

// NOTE: this means the SECOND Id relates to the FIRST Id, e.g.
//     id2 replies to id1
//     id2 reacts to id1
//     id2 deletes id1
//     id2 is a zap receipt on id1

static RELATIONSHIPS_BY_ID1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut RELATIONSHIPS_BY_ID1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_relationships_by_id1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = RELATIONSHIPS_BY_ID1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = RELATIONSHIPS_BY_ID1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = RELATIONSHIPS_BY_ID1_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env.write_txn()?;
                let db = self
                    .env
                    .database_options()
                    .types::<Bytes, Bytes>()
                    // no .flags needed?
                    .name("relationships_by_id1")
                    .create(&mut txn)?;
                txn.commit()?;
                RELATIONSHIPS_BY_ID1_DB = Some(db);
                Ok(db)
            }
        }
    }
}
