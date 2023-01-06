use nostr_types::{Event, Id, IdHex, PublicKeyHex};
use serde::Serialize;
use std::ops::Drop;
use zeroize::Zeroize;

/// This is a message sent to the Overlord
#[derive(Debug, Clone, Serialize)]
pub struct ToOverlordMessage {
    /// What kind of message is this
    pub kind: String,

    /// The payload, serialized as a JSON string
    pub json_payload: String,
}

/// We may send passwords through ToOverlordMessage objects, so we zeroize
/// bus message payloads upon drop.
impl Drop for ToOverlordMessage {
    fn drop(&mut self) {
        self.json_payload.zeroize();
    }
}

/// This is a message sent to the minions
#[derive(Debug, Clone)]
pub struct ToMinionMessage {
    /// The minion we are addressing, based on the URL they are listening to
    /// as a String.  "all" means all minions.
    pub target: String,

    pub payload: ToMinionPayload,
}

#[derive(Debug, Clone)]
pub enum ToMinionPayload {
    Shutdown,
    SubscribeGeneralFeed,
    SubscribePersonFeed(PublicKeyHex),
    SubscribeThreadFeed(Id),
    TempSubscribeMetadata(PublicKeyHex),
    FetchEvents(Vec<IdHex>),
    PostEvent(Box<Event>),
}
