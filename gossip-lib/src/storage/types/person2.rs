use crate::globals::GLOBALS;
use crate::people::PersonList;
use nostr_types::{Metadata, PublicKey};
use serde::{Deserialize, Serialize};

/// A person record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person2 {
    /// Public key
    pub pubkey: PublicKey,

    /// Petname
    pub petname: Option<String>,

    /// Metadata
    pub metadata: Option<Metadata>,

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
    /// for an update)
    pub relay_list_created_at: Option<i64>,

    /// When their relay list was last received (to determine if we need to
    /// check for an update)
    pub relay_list_last_received: i64,
}

impl Person2 {
    pub fn new(pubkey: PublicKey) -> Person2 {
        Person2 {
            pubkey,
            petname: None,
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
        if let Some(md) = &self.metadata {
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

    pub fn tag_name(&self) -> Option<&str> {
        if let Some(pn) = &self.petname {
            Some(pn)
        } else if let Some(md) = &self.metadata {
            if md.other.contains_key("name") {
                if let Some(serde_json::Value::String(s)) = md.other.get("name") {
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

    pub fn is_in_list(&self, list: PersonList) -> bool {
        GLOBALS
            .storage
            .is_person_in_list(&self.pubkey, list)
            .unwrap_or(false)
    }
}

impl PartialEq for Person2 {
    fn eq(&self, other: &Self) -> bool {
        self.pubkey.eq(&other.pubkey)
    }
}
impl Eq for Person2 {}
impl PartialOrd for Person2 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self.tag_name(), other.tag_name()) {
            (Some(a), Some(b)) => a.to_lowercase().partial_cmp(&b.to_lowercase()),
            _ => self.pubkey.partial_cmp(&other.pubkey),
        }
    }
}
impl Ord for Person2 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self.tag_name(), other.tag_name()) {
            (Some(a), Some(b)) => a.to_lowercase().cmp(&b.to_lowercase()),
            _ => self.pubkey.cmp(&other.pubkey),
        }
    }
}
