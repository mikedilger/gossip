use std::ops::Drop;

use serde::Serialize;
use zeroize::Zeroize;

/// This is a message sent between the Overlord and Minions
/// in either direction
#[derive(Debug, Clone, Serialize)]
pub struct BusMessage {
    /// Indended recipient of the message
    pub target: String,

    /// What kind of message is this
    pub kind: String,

    /// The payload, serialized as a JSON string
    pub json_payload: String,
}

/// We may send passwords through BusMessage objects, so we zeroize
/// bus message payloads upon drop.
impl Drop for BusMessage {
    fn drop(&mut self) {
        self.json_payload.zeroize();
    }
}
