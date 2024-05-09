use std::sync::Mutex;

use heed::types::{UnalignedSlice, Unit};
use nostr_types::{EventKind, Id, PublicKey, Unixtime};

use crate::error::{Error, ErrorKind};
use crate::storage::{EmptyDatabase, Storage};

// Author:Kind:Created(reversed):Id -> ()

static EVENT_AKCI_INDEX_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut EVENT_AKCI_INDEX_DB: Option<EmptyDatabase> = None;

impl Storage {
    pub(super) fn db_event_akci_index(&self) -> Result<EmptyDatabase, Error> {
        unsafe {
            if let Some(db) = EVENT_AKCI_INDEX_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = EVENT_AKCI_INDEX_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = EVENT_AKCI_INDEX_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env.write_txn()?;
                let db = self
                    .env
                    .database_options()
                    .types::<UnalignedSlice<u8>, Unit>()
                    .name("event_akci_index")
                    .create(&mut txn)?;
                txn.commit()?;
                EVENT_AKCI_INDEX_DB = Some(db);
                Ok(db)
            }
        }
    }
}

pub struct AkciKey(Vec<u8>);

impl AkciKey {
    pub fn from_parts(author: PublicKey, kind: EventKind, created_at: Unixtime, id: Id) -> Self {
        let mut key: Vec<u8> =
            Vec::with_capacity(32 + std::mem::size_of::<u32>() + std::mem::size_of::<i64>() + 32);
        key.extend(author.as_slice());
        key.extend(u32::from(kind).to_be_bytes());
        key.extend((u64::MAX - created_at.0 as u64).to_be_bytes().as_slice());
        key.extend(id.0.as_slice());
        AkciKey(key)
    }

    pub fn into_parts(self) -> Result<(PublicKey, EventKind, Unixtime, Id), Error> {
        let author = PublicKey::from_bytes(&self.0[..32], true)?;
        let kind: EventKind = u32::from_be_bytes(self.0[32..32 + 4].try_into().unwrap()).into();
        let created_at = Unixtime(
            (u64::MAX - u64::from_be_bytes(self.0[32 + 4..32 + 4 + 8].try_into().unwrap())) as i64,
        );
        let id = Id(self.0[32 + 4 + 8..].try_into().unwrap());
        Ok((author, kind, created_at, id))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<AkciKey, Error> {
        if bytes.len() != 32 + std::mem::size_of::<u32>() + std::mem::size_of::<i64>() + 32 {
            return Err(ErrorKind::KeySizeWrong.into());
        }
        Ok(AkciKey(bytes.to_owned()))
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use nostr_types::PrivateKey;

    use super::*;

    #[test]
    fn test_event_akci_key() {
        let pubkey = PrivateKey::generate().public_key();
        let kind = EventKind::TextNote;
        let created_at = Unixtime::now().unwrap();
        let id = Id::try_from_hex_string(
            "77f7653c67147a125cc624f695029d0557e3ab402e714680eb23dd2499f439a0",
        )
        .unwrap();

        let key = AkciKey::from_parts(pubkey, kind, created_at, id);
        let (pubkey2, kind2, created_at2, id2) = key.into_parts().unwrap();

        assert_eq!(pubkey, pubkey2);
        assert_eq!(kind, kind2);
        assert_eq!(created_at, created_at2);
        assert_eq!(id, id2);
    }
}
