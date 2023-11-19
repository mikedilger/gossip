use crate::error::Error;
use crate::relationship::Relationship;
use crate::storage::{RawDatabase, Storage};
use heed::RwTxn;
use heed::{types::UnalignedSlice, DatabaseFlags};
use nostr_types::{EventAddr, Id};
use speedy::{Readable, Writable};
use std::sync::Mutex;

// Kind:Pubkey:d-tag -> Relationship:Id

static REPREL1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut REPREL1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_reprel1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = REPREL1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = REPREL1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = REPREL1_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env.write_txn()?;
                let db = self
                    .env
                    .database_options()
                    .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
                    .flags(DatabaseFlags::DUP_SORT) // NOT FIXED, Relationship serialized isn't.
                    .name("reprel1")
                    .create(&mut txn)?;
                txn.commit()?;
                REPREL1_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub(crate) fn write_reprel1<'a>(
        &'a self,
        addr: EventAddr,
        related: Id,
        relationship: Relationship,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let key = reprel1_into_key(&addr);
        let value = reprel1_into_value(relationship, related)?;
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_reprel1()?.put(txn, &key, &value)?;
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

    pub(crate) fn find_reprels1(&self, addr: &EventAddr) -> Result<Vec<(Id, Relationship)>, Error> {
        let key = reprel1_into_key(addr);
        let txn = self.env.read_txn()?;
        let iter = match self.db_reprel1()?.get_duplicates(&txn, &key)? {
            Some(iter) => iter,
            None => return Ok(vec![]),
        };
        let mut output: Vec<(Id, Relationship)> = Vec::new();
        for result in iter {
            let (_key, val) = result?;
            let (rel, id) = reprel1_from_value(val)?;
            output.push((id, rel));
        }
        Ok(output)
    }
}

fn reprel1_into_key(ea: &EventAddr) -> Vec<u8> {
    let u: u32 = ea.kind.into();
    let mut key: Vec<u8> = u.to_be_bytes().as_slice().to_owned();
    key.extend(ea.author.as_bytes());
    key.extend(ea.d.as_bytes());
    key
}

/*
fn reprel1_from_key(key: &[u8]) -> Result<EventAddr, Error> {
    let u = u32::from_be_bytes(key[..4].try_into().unwrap());
    let kind: EventKind = u.into();
    let pubkey: PublicKey = PublicKey::from_bytes(&key[4..4+32], true)?;
    let d: String = String::from_utf8_lossy(&key[4+32..]).to_string();
    Ok(EventAddr {
        d,
        relays: vec![],
        kind,
        author: pubkey
    })
}
 */

fn reprel1_into_value(relationship: Relationship, id: Id) -> Result<Vec<u8>, Error> {
    let mut value: Vec<u8> = relationship.write_to_vec()?;
    value.extend(id.as_slice());
    Ok(value)
}

fn reprel1_from_value(value: &[u8]) -> Result<(Relationship, Id), Error> {
    let (result, len) = Relationship::read_with_length_from_buffer(value);
    let relationship = result?;
    let id = Id(value[len..len + 32].try_into().unwrap());
    Ok((relationship, id))
}
