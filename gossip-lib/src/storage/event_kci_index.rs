use crate::error::{Error, ErrorKind};
use crate::storage::{EmptyDatabase, Storage};
use heed::types::{Bytes, Unit};
use nostr_types::{EventKind, Id, Unixtime};
use std::sync::Mutex;

// Kind:Created(reversed):Id -> ()

static EVENT_KCI_INDEX_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut EVENT_KCI_INDEX_DB: Option<EmptyDatabase> = None;

pub(super) const INDEXED_KINDS: [EventKind; 6] = [
    EventKind::Metadata,
    EventKind::ContactList,
    EventKind::RelayList,
    EventKind::DmRelayList,
    EventKind::EncryptedDirectMessage,
    EventKind::GiftWrap,
];

impl Storage {
    pub(super) fn db_event_kci_index(&self) -> Result<EmptyDatabase, Error> {
        unsafe {
            if let Some(db) = EVENT_KCI_INDEX_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = EVENT_KCI_INDEX_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = EVENT_KCI_INDEX_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env().write_txn()?;
                let db = self
                    .env()
                    .database_options()
                    .types::<Bytes, Unit>()
                    .name("event_kci_index")
                    .create(&mut txn)?;
                txn.commit()?;
                EVENT_KCI_INDEX_DB = Some(db);
                Ok(db)
            }
        }
    }
}

pub struct KciKey(Vec<u8>);

impl KciKey {
    pub fn from_parts(kind: EventKind, created_at: Unixtime, id: Id) -> Self {
        let mut key: Vec<u8> =
            Vec::with_capacity(std::mem::size_of::<u32>() + std::mem::size_of::<i64>() + 32);
        key.extend(u32::from(kind).to_be_bytes());
        key.extend((u64::MAX - created_at.0 as u64).to_be_bytes().as_slice());
        key.extend(id.0.as_slice());
        KciKey(key)
    }

    pub fn into_parts(self) -> Result<(EventKind, Unixtime, Id), Error> {
        let kind: EventKind = u32::from_be_bytes(self.0[0..4].try_into().unwrap()).into();
        let created_at =
            Unixtime((u64::MAX - u64::from_be_bytes(self.0[4..4 + 8].try_into().unwrap())) as i64);
        let id = Id(self.0[4 + 8..].try_into().unwrap());
        Ok((kind, created_at, id))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<KciKey, Error> {
        if bytes.len() != std::mem::size_of::<u32>() + std::mem::size_of::<i64>() + 32 {
            return Err(ErrorKind::KeySizeWrong.into());
        }
        Ok(KciKey(bytes.to_owned()))
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_event_kci_key() {
        let kind = EventKind::TextNote;
        let created_at = Unixtime::now();
        let id = Id::try_from_hex_string(
            "77f7653c67147a125cc624f695029d0557e3ab402e714680eb23dd2499f439a0",
        )
        .unwrap();

        let key = KciKey::from_parts(kind, created_at, id);
        let (kind2, created_at2, id2) = key.into_parts().unwrap();

        assert_eq!(kind, kind2);
        assert_eq!(created_at, created_at2);
        assert_eq!(id, id2);
    }
}
