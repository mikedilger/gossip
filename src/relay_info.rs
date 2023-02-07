use crate::db::DbRelay;
use nostr_types::PublicKeyHex;

/// All the per-relay information we need kept hot in memory
#[derive(Debug, Clone)]
pub struct RelayInfo {
    pub dbrelay: DbRelay,
    pub connected: bool,
    pub assignment: Option<RelayAssignment>,
    //pub subscriptions: Vec<String>,
}

/// A RelayAssignment is a record of a relay which is serving (or will serve) the general
/// feed for a set of public keys.
#[derive(Debug, Clone)]
pub struct RelayAssignment {
    pub relay: DbRelay,
    pub pubkeys: Vec<PublicKeyHex>,
}
