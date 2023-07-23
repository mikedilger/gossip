use nostr_types::{MilliSatoshi, PublicKey};
use speedy::{Readable, Writable};

/// A relationship between events
#[derive(Clone, Debug, PartialEq, Eq, Readable, Writable)]
pub enum Relationship {
    Reply,
    Reaction(PublicKey, String),
    Deletion(String),
    ZapReceipt(PublicKey, MilliSatoshi),
}
