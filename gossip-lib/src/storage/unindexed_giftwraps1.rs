use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::storage::{RawDatabase, Storage};
use heed::types::Bytes;
use nostr_types::Id;
use std::sync::Mutex;

// Id -> ()
//   key: id.as_slice()
//   val: vec![]

static UNINDEXED_GIFTWRAPS1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut UNINDEXED_GIFTWRAPS1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_unindexed_giftwraps1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = UNINDEXED_GIFTWRAPS1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = UNINDEXED_GIFTWRAPS1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = UNINDEXED_GIFTWRAPS1_DB {
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
                    .name("unindexed_giftwraps")
                    .create(&mut txn)?;
                txn.commit()?;
                UNINDEXED_GIFTWRAPS1_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub(crate) async fn index_unindexed_giftwraps1(&self) -> Result<(), Error> {
        if !GLOBALS.identity.is_unlocked() {
            return Err(ErrorKind::NoPrivateKey.into());
        }

        let mut ids: Vec<Id> = Vec::new();
        let txn = self.env.read_txn()?;
        let iter = self.db_unindexed_giftwraps1()?.iter(&txn)?;
        for result in iter {
            let (key, _val) = result?;
            let a: [u8; 32] = key.try_into()?;
            let id = Id(a);
            ids.push(id);
        }

        let mut txn = self.env.write_txn()?;
        for id in ids {
            if let Some(event) = self.read_event(id)? {
                self.write_event_akci_index(
                    event.pubkey,
                    event.kind,
                    event.created_at,
                    event.id,
                    Some(&mut txn),
                )?;
                self.write_event_kci_index(event.kind, event.created_at, event.id, Some(&mut txn))?;
                self.write_event_tci_index(&event, Some(&mut txn)).await?;
            }
            self.db_unindexed_giftwraps1()?
                .delete(&mut txn, id.as_slice())?;
        }

        txn.commit()?;

        Ok(())
    }
}
