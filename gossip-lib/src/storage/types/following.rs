use super::{ByteRep, Record};
use crate::error::Error;
use nostr_types::PublicKey;
use serde::{Deserialize, Serialize};
use speedy::{Readable, Writable};

// THIS IS HISTORICAL FOR MIGRATIONS AND THE STRUCTURES SHOULD NOT BE EDITED

/// A pubkey record
#[derive(Debug, Clone, Readable, Writable, Serialize, Deserialize)]
pub struct Following {
    /// The person doing the following
    pub actor: PublicKey,

    /// The people they are following
    pub followed: Vec<PublicKey>,
}

impl Following {
    pub fn new(actor: PublicKey, pubkeys: Vec<PublicKey>) -> Following {
        Following {
            actor,
            followed: pubkeys,
        }
    }
}

impl ByteRep for Following {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.write_to_vec()?)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Self::read_from_buffer(bytes)?)
    }
}

impl Record for Following {
    type Key = PublicKey;

    /// Create a new record
    fn new(k: Self::Key) -> Option<Self> {
        Some(Following {
            actor: k,
            followed: vec![],
        })
    }

    // Get the key of a record
    fn key(&self) -> Self::Key {
        self.actor
    }
}
