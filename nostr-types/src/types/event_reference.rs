use super::{Id, NAddr, PublicKey, RelayUrl};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

/// A reference to another event, either by `Id` (often coming from an 'e' tag),
/// or by `NAddr` (often coming from an 'a' tag).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EventReference {
    /// Refer to a specific event by Id
    Id {
        /// The event id
        id: Id,

        /// Optionally include author (to find via their relay list)
        author: Option<PublicKey>,

        /// Optionally include relays (to find the event)
        relays: Vec<RelayUrl>,

        /// Optional marker, if this came from an event tag
        marker: Option<String>,
    },

    /// Refer to a replaceable event by NAddr
    Addr(NAddr),
}

impl EventReference {
    /// Get the author
    pub fn author(&self) -> Option<PublicKey> {
        match self {
            EventReference::Id { author, .. } => *author,
            EventReference::Addr(naddr) => Some(naddr.author),
        }
    }

    /// Set the author
    pub fn set_author(&mut self, new_author: PublicKey) {
        match self {
            EventReference::Id { ref mut author, .. } => *author = Some(new_author),
            EventReference::Addr(ref mut naddr) => naddr.author = new_author,
        }
    }

    /// Copy the relays
    pub fn copy_relays(&self) -> Vec<RelayUrl> {
        match self {
            EventReference::Id { relays, .. } => relays.clone(),
            EventReference::Addr(naddr) => naddr
                .relays
                .iter()
                .filter_map(|r| RelayUrl::try_from_unchecked_url(r).ok())
                .collect(),
        }
    }

    /// Extend relays
    pub fn extend_relays(&mut self, relays: Vec<RelayUrl>) {
        let mut new_relays = self.copy_relays();
        new_relays.extend(relays);

        match self {
            EventReference::Id { ref mut relays, .. } => *relays = new_relays,
            EventReference::Addr(ref mut naddr) => {
                naddr.relays = new_relays.iter().map(|r| r.to_unchecked_url()).collect()
            }
        }
    }
}

impl PartialEq for EventReference {
    fn eq(&self, other: &Self) -> bool {
        match self {
            EventReference::Id { id: id1, .. } => {
                match other {
                    EventReference::Id { id: id2, .. } => {
                        // We don't compare the other fields which are only helpers,
                        // not definitive identity
                        id1 == id2
                    }
                    _ => false,
                }
            }
            EventReference::Addr(addr1) => match other {
                EventReference::Addr(addr2) => addr1 == addr2,
                _ => false,
            },
        }
    }
}

impl Eq for EventReference {}

impl Hash for EventReference {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            EventReference::Id { id, .. } => {
                // We do not hash the other fields which are only helpers,
                // not definitive identity
                id.hash(state);
            }
            EventReference::Addr(addr) => {
                addr.hash(state);
            }
        }
    }
}
