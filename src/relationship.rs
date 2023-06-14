use nostr_types::MilliSatoshi;

/// A relationship between events
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Relationship {
    Root,
    Reply,
    Mention,
    Reaction(String),
    Deletion(String),
    ZapReceipt(MilliSatoshi),
}
