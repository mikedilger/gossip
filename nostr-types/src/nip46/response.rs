use serde::{Deserialize, Serialize};

/// A NIP-46 request, found stringified in the content of a kind 24133 event
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Nip46Response {
    /// The Request Id being responded to
    pub id: String,

    /// The result, either a string or a stringified JSON object
    pub result: String,

    /// Optionally an error (in which case result is usually empty)
    pub error: Option<String>,
}
