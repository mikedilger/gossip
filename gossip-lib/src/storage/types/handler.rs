use super::{ByteRep, Record};
use crate::error::Error;
use nostr_types::{Event, EventKind, Metadata, PublicKey, UncheckedUrl};
use serde::{Deserialize, Serialize};
use speedy::{Readable, Writable};
use std::sync::OnceLock;

// THIS IS HISTORICAL FOR MIGRATIONS AND THE STRUCTURES SHOULD NOT BE EDITED

/// This is a key into the Handler table identifying the app and the 'd' tag on their
/// handler event (an app can have multiple handler events with different 'd' tags)
#[derive(Debug, Clone, Readable, Writable, Serialize, Deserialize, PartialEq)]
pub struct HandlerKey {
    /// Public key
    pub pubkey: PublicKey,

    /// d tag
    pub d: String,
}

impl ByteRep for HandlerKey {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.write_to_vec()?)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Self::read_from_buffer(bytes)?)
    }
}

/// A handler record
#[derive(Debug, Clone, Readable, Writable, Serialize, Deserialize)]
pub struct Handler {
    /// Handler key
    pub key: HandlerKey,

    /// Metadata serialized as JSON
    pub(in crate::storage) metadata_json: String,

    // We deserialize metadata on first access
    #[serde(skip)]
    #[speedy(skip)]
    pub(in crate::storage) deserialized_metadata: OnceLock<Option<Metadata>>,

    /// Event kinds handled
    pub kinds: Vec<EventKind>,

    /// URL handling nevent (web only)
    pub nevent_url: Option<UncheckedUrl>,

    /// URL handling naddr (web only)
    pub naddr_url: Option<UncheckedUrl>,
}

impl Handler {
    pub fn from_31990(event: &Event) -> Option<Handler> {
        if event.kind != EventKind::HandlerInformation {
            return None;
        }

        let mut d = "".to_owned();
        let mut nevent_url = None;
        let mut naddr_url = None;
        let mut kinds = Vec::new();

        for tag in &event.tags {
            if tag.get_index(0) == "d" {
                d = tag.get_index(1).to_owned();
            } else if tag.get_index(0) == "k" {
                if let Ok(kindnum) = tag.get_index(1).parse::<u32>() {
                    let kind: EventKind = kindnum.into();
                    kinds.push(kind);
                }
            } else if tag.get_index(0) == "web" {
                if tag.get_index(2) == "nevent" {
                    nevent_url = Some(UncheckedUrl::from_str(tag.get_index(1)));
                } else if tag.get_index(2) == "naddr" {
                    naddr_url = Some(UncheckedUrl::from_str(tag.get_index(1)));
                }
            }
        }

        if kinds.is_empty() {
            return None;
        }

        // Don't store it if it doesn't handle anything useful.
        if nevent_url.is_none() && naddr_url.is_none() {
            return None;
        }

        let handler = Handler {
            key: HandlerKey {
                pubkey: event.pubkey,
                d,
            },
            metadata_json: event.content.clone(),
            deserialized_metadata: OnceLock::new(),
            kinds,
            nevent_url,
            naddr_url,
        };

        // If it doesn't have a valid hostname for either nevent or naddr:
        if handler.hostname(EventKind::TextNote).is_none()
            && handler.hostname(EventKind::LongFormContent).is_none()
        {
            None
        } else {
            Some(handler)
        }
    }

    pub fn metadata(&self) -> &Option<Metadata> {
        self.deserialized_metadata
            .get_or_init(|| serde_json::from_str::<Metadata>(&self.metadata_json).ok())
    }

    pub fn bestname(&self, kind: EventKind) -> Option<String> {
        match self.metaname() {
            Some(n) => Some(n),
            None => self.hostname(kind),
        }
    }

    pub fn metaname(&self) -> Option<String> {
        // Try metadata
        if let Some(m) = self.metadata() {
            if let Some(n) = &m.name {
                return Some(n.to_owned());
            }
        }

        None
    }

    pub fn hostname(&self, kind: EventKind) -> Option<String> {
        if kind.is_parameterized_replaceable() {
            if let Some(url) = &self.naddr_url {
                if let Ok(uri) = url.as_str().replace("<naddr>", "x").parse::<http::Uri>() {
                    if let Some(host) = uri.host() {
                        return Some(host.to_owned());
                    }
                }
            }
        } else {
            if let Some(url) = &self.nevent_url {
                if let Ok(uri) = url.as_str().replace("<naddr>", "x").parse::<http::Uri>() {
                    if let Some(host) = uri.host() {
                        return Some(host.to_owned());
                    }
                }
            }
        }

        None
    }
}

impl ByteRep for Handler {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.write_to_vec()?)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Self::read_from_buffer(bytes)?)
    }
}

impl Record for Handler {
    type Key = HandlerKey;

    /// Create a new default record if possible
    fn new(_k: Self::Key) -> Option<Self> {
        None
    }

    /// Get the key of a record
    fn key(&self) -> Self::Key {
        self.key.clone()
    }
}
