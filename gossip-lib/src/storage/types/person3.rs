use super::{ByteRep, Record};
use crate::error::Error;
use crate::globals::GLOBALS;
use crate::people::PersonList;
use nostr_types::{Metadata, PublicKey};
use serde::{Deserialize, Serialize};
use speedy::{Readable, Writable};
use std::sync::OnceLock;

// THIS IS HISTORICAL FOR MIGRATIONS AND THE STRUCTURES SHOULD NOT BE EDITED

/// A person record
#[derive(Debug, Clone, Readable, Writable, Serialize, Deserialize)]
pub struct Person3 {
    /// Public key
    pub pubkey: PublicKey,

    /// Petname
    pub petname: Option<String>,

    /// Metadata serialized as JSON
    pub(in crate::storage) metadata_json: Option<String>,

    // We deserialize metadata on first access
    //
    // We reserialize it with Record::stabilize() prior to Table writing.
    // if this is empty, it hasn't been deserialized yet
    #[serde(skip)]
    #[speedy(skip)]
    pub(in crate::storage) deserialized_metadata: OnceLock<Option<Metadata>>,

    /// When the metadata was created
    pub metadata_created_at: Option<i64>,

    /// When the metadata was last received (to determine if we need to check
    /// for an update)
    pub metadata_last_received: i64,

    /// If nip05 checked out to be valid
    pub nip05_valid: bool,

    /// When the nip05 was last checked (to determine if we need to check again)
    pub nip05_last_checked: Option<u64>,

    /// When their relay list was created (to determine if we need to check
    /// for an update, and if a list is newer than what we've had before)
    pub relay_list_created_at: Option<i64>,

    /// When their relay list was last sought (to determine if we need to
    /// check for an update)
    #[serde(rename = "relay_list_last_received")]
    pub relay_list_last_sought: i64,

    /// When their dm relay list was created (to determine if we need to check
    /// for an update)
    pub dm_relay_list_created_at: Option<i64>,

    /// When their dm relay list was last sought (to determine if we need to
    /// check for an update)
    pub dm_relay_list_last_sought: i64,
}

impl Person3 {
    pub fn new(pubkey: PublicKey) -> Person3 {
        Person3 {
            pubkey,
            petname: None,
            metadata_json: None,
            deserialized_metadata: OnceLock::new(),
            metadata_created_at: None,
            metadata_last_received: 0,
            nip05_valid: false,
            nip05_last_checked: None,
            relay_list_created_at: None,
            relay_list_last_sought: 0,
            dm_relay_list_created_at: None,
            dm_relay_list_last_sought: 0,
        }
    }

    pub fn metadata(&self) -> Option<&Metadata> {
        self.deserialized_metadata.get_or_init(|| {
            match &self.metadata_json {
                None => None,
                Some(s) => serde_json::from_str::<Metadata>(s).ok()
            }
        }).as_ref()
    }

    pub fn metadata_mut(&mut self) -> &mut Option<Metadata> {
        if self.deserialized_metadata.get().is_none() {
            let md = match &self.metadata_json {
                None => None,
                Some(s) => serde_json::from_str::<Metadata>(s).ok()
            };
            self.deserialized_metadata.set(md).unwrap();
        }

        self.deserialized_metadata.get_mut().unwrap()
    }

    pub fn best_name(&self) -> String {
        if let Some(pn) = &self.petname {
            return pn.to_owned();
        }
        if let Some(md) = self.metadata() {
            if let Some(n) = &md.name {
                if !n.is_empty() {
                    return n.to_owned();
                }
            }
            if let Some(serde_json::Value::String(s)) = md.other.get("display_name") {
                if !s.is_empty() {
                    return s.to_owned();
                }
            }
        }
        crate::names::pubkey_short(&self.pubkey)
    }

    pub fn name(&self) -> Option<&str> {
        if let Some(md) = self.metadata() {
            md.name.as_deref()
        } else {
            None
        }
    }

    pub fn about(&self) -> Option<&str> {
        if let Some(md) = self.metadata() {
            md.about.as_deref()
        } else {
            None
        }
    }

    pub fn picture(&self) -> Option<&str> {
        if let Some(md) = self.metadata() {
            md.picture.as_deref()
        } else {
            None
        }
    }

    pub fn display_name(&self) -> Option<&str> {
        if let Some(md) = self.metadata() {
            if md.other.contains_key("display_name") {
                if let Some(serde_json::Value::String(s)) = md.other.get("display_name") {
                    if !s.is_empty() {
                        return Some(s);
                    }
                }
            }
        }
        None
    }

    pub fn nip05(&self) -> Option<&str> {
        if let Some(md) = self.metadata() {
            md.nip05.as_deref()
        } else {
            None
        }
    }

    pub fn is_in_list(&self, list: PersonList) -> bool {
        GLOBALS
            .storage
            .is_person_in_list(&self.pubkey, list)
            .unwrap_or(false)
    }

    pub fn is_subscribed_to(&self) -> bool {
        GLOBALS
            .storage
            .is_person_subscribed_to(&self.pubkey)
            .unwrap_or(false)
    }
}

impl PartialEq for Person3 {
    fn eq(&self, other: &Self) -> bool {
        self.pubkey.eq(&other.pubkey)
    }
}
impl Eq for Person3 {}
impl PartialOrd for Person3 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Person3 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.best_name()
            .to_lowercase()
            .cmp(&other.best_name().to_lowercase())
    }
}

impl ByteRep for Person3 {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.write_to_vec()?)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Self::read_from_buffer(bytes)?)
    }
}

impl Record for Person3 {
    type Key = PublicKey;

    /// Create a new record
    fn new(k: Self::Key) -> Self {
        Person3::new(k)
    }

    /// Get the key of a record
    fn key(&self) -> Self::Key {
        self.pubkey
    }

    /// Stabilize
    fn stabilize(&mut self) {
        if let Some(dm) = self.deserialized_metadata.get() {
            if let Ok(s) = serde_json::to_string(dm) {
                self.metadata_json = Some(s);
            }
        }
    }
}
