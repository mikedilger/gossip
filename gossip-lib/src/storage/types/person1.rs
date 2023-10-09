use nostr_types::{Metadata, PublicKey};
use serde::{Deserialize, Serialize};

// THIS IS HISTORICAL FOR MIGRATIONS AND THE STRUCTURES SHOULD NOT BE EDITED

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

#[allow(dead_code)]
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
