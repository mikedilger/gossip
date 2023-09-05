use crate::error::Error;
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::{Metadata, PublicKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person1 {
    pub pubkey: PublicKey,
    pub petname: Option<String>,
    pub followed: bool,
    pub followed_last_updated: i64,
    pub muted: bool,
    pub metadata: Option<Metadata>,
    pub metadata_created_at: Option<i64>,
    pub metadata_last_received: i64,
    pub nip05_valid: bool,
    pub nip05_last_checked: Option<u64>,
    pub relay_list_created_at: Option<i64>,
    pub relay_list_last_received: i64,
}

impl Person1 {
    pub fn new(pubkey: PublicKey) -> Person1 {
        Person1 {
            pubkey,
            petname: None,
            followed: false,
            followed_last_updated: 0,
            muted: false,
            metadata: None,
            metadata_created_at: None,
            metadata_last_received: 0,
            nip05_valid: false,
            nip05_last_checked: None,
            relay_list_created_at: None,
            relay_list_last_received: 0,
        }
    }

    pub fn display_name(&self) -> Option<&str> {
        if let Some(pn) = &self.petname {
            Some(pn)
        } else if let Some(md) = &self.metadata {
            if md.other.contains_key("display_name") {
                if let Some(serde_json::Value::String(s)) = md.other.get("display_name") {
                    if !s.is_empty() {
                        return Some(s);
                    }
                }
            }
            md.name.as_deref()
        } else {
            None
        }
    }

    pub fn name(&self) -> Option<&str> {
        if let Some(md) = &self.metadata {
            md.name.as_deref()
        } else {
            None
        }
    }

    pub fn about(&self) -> Option<&str> {
        if let Some(md) = &self.metadata {
            md.about.as_deref()
        } else {
            None
        }
    }

    pub fn picture(&self) -> Option<&str> {
        if let Some(md) = &self.metadata {
            md.picture.as_deref()
        } else {
            None
        }
    }

    pub fn nip05(&self) -> Option<&str> {
        if let Some(md) = &self.metadata {
            md.nip05.as_deref()
        } else {
            None
        }
    }
}

impl PartialEq for Person1 {
    fn eq(&self, other: &Self) -> bool {
        self.pubkey.eq(&other.pubkey)
    }
}
impl Eq for Person1 {}
impl PartialOrd for Person1 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self.display_name(), other.display_name()) {
            (Some(a), Some(b)) => a.to_lowercase().partial_cmp(&b.to_lowercase()),
            _ => self.pubkey.partial_cmp(&other.pubkey),
        }
    }
}
impl Ord for Person1 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self.display_name(), other.display_name()) {
            (Some(a), Some(b)) => a.to_lowercase().cmp(&b.to_lowercase()),
            _ => self.pubkey.cmp(&other.pubkey),
        }
    }
}

impl Storage {
    pub fn get_people1_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.people.len(&txn)?)
    }

    #[allow(dead_code)]
    pub fn write_person1<'a>(
        &'a self,
        person: &Person1,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key: Vec<u8> = person.pubkey.to_bytes();
        let bytes = serde_json::to_vec(person)?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.people.put(txn, &key, &bytes)?;
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

    pub fn read_person1(&self, pubkey: &PublicKey) -> Result<Option<Person1>, Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key: Vec<u8> = pubkey.to_bytes();
        let txn = self.env.read_txn()?;
        Ok(match self.people.get(&txn, &key)? {
            Some(bytes) => Some(serde_json::from_slice(bytes)?),
            None => None,
        })
    }

    pub fn filter_people1<F>(&self, f: F) -> Result<Vec<Person1>, Error>
    where
        F: Fn(&Person1) -> bool,
    {
        let txn = self.env.read_txn()?;
        let iter = self.people.iter(&txn)?;
        let mut output: Vec<Person1> = Vec::new();
        for result in iter {
            let (_key, val) = result?;
            let person: Person1 = serde_json::from_slice(val)?;
            if f(&person) {
                output.push(person);
            }
        }
        Ok(output)
    }
}
