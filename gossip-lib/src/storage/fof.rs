use crate::error::Error;
use crate::globals::GLOBALS;
use crate::storage::{FollowingsTable, RawDatabase, Storage, Table};
use crate::PersonList;
use heed::types::Bytes;
use heed::RwTxn;
use nostr_types::{EventKind, Filter, PublicKey};
use std::sync::Mutex;

// Pubkey -> u64
//   key: key!(pubkey.as_bytes())
//   val: u64.as_be_bytes();

static FOF_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut FOF_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_fof(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = FOF_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = FOF_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = FOF_DB {
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
                    .name("wot")
                    .create(&mut txn)?;
                txn.commit()?;
                FOF_DB = Some(db);
                Ok(db)
            }
        }
    }

    // Write fof
    #[allow(dead_code)]
    pub(crate) fn write_fof<'a>(
        &'a self,
        pubkey: PublicKey,
        fof: u64,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        self.db_fof()?
            .put(txn, pubkey.as_bytes(), fof.to_be_bytes().as_slice())?;

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }

    // Read fof
    pub fn read_fof(&self, pubkey: PublicKey) -> Result<u64, Error> {
        let txn = self.get_read_txn()?;
        let fof = match self.db_fof()?.get(&txn, pubkey.as_bytes())? {
            Some(bytes) => u64::from_be_bytes(<[u8; 8]>::try_from(&bytes[..8]).unwrap()),
            None => 0,
        };
        Ok(fof)
    }

    // Incr fof
    pub(crate) fn incr_fof<'a>(
        &'a self,
        pubkey: PublicKey,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        let mut fof = match self.db_fof()?.get(txn, pubkey.as_bytes())? {
            Some(bytes) => u64::from_be_bytes(<[u8; 8]>::try_from(&bytes[..8]).unwrap()),
            None => 0,
        };
        fof += 1;
        self.db_fof()?
            .put(txn, pubkey.as_bytes(), fof.to_be_bytes().as_slice())?;

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }

    // Decr fof
    pub(crate) fn decr_fof<'a>(
        &'a self,
        pubkey: PublicKey,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        let mut fof = match self.db_fof()?.get(txn, pubkey.as_bytes())? {
            Some(bytes) => u64::from_be_bytes(<[u8; 8]>::try_from(&bytes[..8]).unwrap()),
            None => 0,
        };
        fof = fof.saturating_sub(1);
        self.db_fof()?
            .put(txn, pubkey.as_bytes(), fof.to_be_bytes().as_slice())?;

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }

    pub(crate) fn rebuild_fof<'a>(&'a self, rw_txn: Option<&mut RwTxn<'a>>) -> Result<(), Error> {
        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        // Clear Fof data
        self.db_fof()?.clear(txn)?;

        // Clear following lists
        FollowingsTable::clear(Some(txn))?;

        // Get the contact lists of each person we follow
        let mut filter = Filter::new();
        filter.add_event_kind(EventKind::ContactList);
        for pubkey in GLOBALS
            .db()
            .get_people_in_list(PersonList::Followed)?
            .iter()
            .map(|(pk, _private)| pk)
        {
            filter.add_author(*pubkey);
        }
        let contact_lists = self.find_events_by_filter(&filter, |_| true)?;

        for event in &contact_lists {
            crate::process::update_followings_and_fof_from_contact_list(event, Some(txn))?;
        }

        self.set_flag_rebuild_fof_needed(false, Some(txn))?;

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }
}
