use nostr_types::MilliSatoshi;
use speedy::{Readable, Writable};

/// A relationship between events
#[derive(Clone, Debug, PartialEq, Eq, Readable, Writable)]
pub enum Relationship {
    //Root,
    Reply,
    //Mention,
    Reaction(String),
    Deletion(String),
    ZapReceipt(MilliSatoshi),
}
