use crate::error::{Error, ErrorKind};
use crate::storage::{EmptyDatabase, Storage};
use heed::types::{Bytes, Unit};
use heed::RwTxn;
use nostr_types::{EventKind, EventV3, Id, PublicKeyHex, TagV3, Unixtime};
use std::sync::Mutex;

// This replaces event_tci_index which didn't have the
// reverse created_at suffix.

pub(super) const INDEXED_TAGS: [&str; 4] = ["a", "d", "p", "delegation"];

// This indexes these tags, except for "p" tags we only index it if
//   1) the "p" tag is our user, or
//   2) the event is a ContactList

// TagKey:QUOTE:TagValue:QUOTE:Created(reversed):Id -> ()

static EVENT_TCI_INDEX_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut EVENT_TCI_INDEX_DB: Option<EmptyDatabase> = None;

impl Storage {
    pub(super) fn db_event_tci_index(&self) -> Result<EmptyDatabase, Error> {
        unsafe {
            if let Some(db) = EVENT_TCI_INDEX_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = EVENT_TCI_INDEX_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = EVENT_TCI_INDEX_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env.write_txn()?;
                let db = self
                    .env
                    .database_options()
                    .types::<Bytes, Unit>()
                    .name("event_tci_index")
                    .create(&mut txn)?;
                txn.commit()?;
                EVENT_TCI_INDEX_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub fn write_event3_tci_index<'a>(
        &'a self,
        event: &EventV3,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut local_txn = None;
        let txn = maybe_local_txn!(self, rw_txn, local_txn);

        // If giftwrap:
        //   Use the id and kind of the giftwrap,
        //   Use the pubkey and created_at of the rumor
        let mut innerevent: &EventV3 = event;
        let rumor: EventV3;
        if let Some(r) = self.switch_to_rumor3(event, txn)? {
            rumor = r;
            innerevent = &rumor;
        }

        // our user's public key
        let pk: Option<PublicKeyHex> = self.read_setting_public_key().map(|p| p.into());

        // Index tags from giftwrap and rumor
        let mut tags: Vec<TagV3> = event.tags.clone();
        if innerevent != event {
            tags.append(&mut innerevent.tags.clone());
        }

        for tag in &tags {
            let tagname = tag.tagname();
            let value = tag.value();
            if value.is_empty() {
                continue; // no tag value, not indexable.
            }

            // Only index tags we intend to lookup later by tag.
            // If that set changes, (1) add to this code and (2) do a reindex migration
            if !INDEXED_TAGS.contains(&tagname) {
                continue;
            }
            // For 'p' tags, only index them if 'p' is our user, or if the event is
            // a ContactList
            if tagname == "p" {
                if event.kind != EventKind::ContactList {
                    match &pk {
                        None => continue,
                        Some(pk) => {
                            if value != pk.as_str() {
                                continue;
                            }
                        }
                    }
                }
            }

            let key = TciKey::from_parts(tagname, value, event.created_at, event.id);
            self.db_event_tci_index()?.put(txn, key.as_slice(), &())?;
        }

        maybe_local_txn_commit!(local_txn);

        Ok(())
    }
}

pub struct TciKey(Vec<u8>);

impl TciKey {
    pub fn from_parts(tagname: &str, tagvalue: &str, created_at: Unixtime, id: Id) -> Self {
        let mut key: Vec<u8> = Vec::with_capacity(
            tagname.len() + 1 + tagvalue.len() + std::mem::size_of::<i64>() + 32,
        );
        key.extend(tagname.as_bytes());
        key.push(b'\"');
        key.extend(tagvalue.as_bytes());
        key.extend((u64::MAX - created_at.0 as u64).to_be_bytes().as_slice());
        key.extend(id.0.as_slice());
        TciKey(key)
    }

    pub fn into_parts(self) -> Result<(String, String, Unixtime, Id), Error> {
        let q = self
            .0
            .iter()
            .position(|b| *b == b'\"')
            .ok_or::<Error>(ErrorKind::KeyInvalid.into())?;
        let mid = self.0.len() - 32 - std::mem::size_of::<i64>();
        let tagname = String::from_utf8_lossy(&self.0[0..q]).into_owned();
        let tagval = String::from_utf8_lossy(&self.0[q + 1..mid]).into_owned();
        let created_at = Unixtime(
            (u64::MAX - u64::from_be_bytes(self.0[mid..mid + 8].try_into().unwrap())) as i64,
        );
        let id = Id(self.0[mid + 8..].try_into().unwrap());
        Ok((tagname.to_string(), tagval.to_string(), created_at, id))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<TciKey, Error> {
        if bytes.len() < 4 + std::mem::size_of::<i64>() + 32 {
            return Err(ErrorKind::KeySizeWrong.into());
        }
        // still might be wrong, but that is all we are checking for now.

        Ok(TciKey(bytes.to_owned()))
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_event_tci_key() {
        let tagname = "p";
        let tagval = "ee11a5dff40c19a555f41fe42b48f00e618c91225622ae37b6c2bb67b76c4e49";
        let created_at = Unixtime::now();
        let id = Id::try_from_hex_string(
            "77f7653c67147a125cc624f695029d0557e3ab402e714680eb23dd2499f439a0",
        )
        .unwrap();

        let key = TciKey::from_parts(tagname, tagval, created_at, id);
        let (tagname2, tagval2, created_at2, id2) = key.into_parts().unwrap();

        assert_eq!(tagname, tagname2);
        assert_eq!(tagval, tagval2);
        assert_eq!(created_at, created_at2);
        assert_eq!(id, id2);
    }
}
