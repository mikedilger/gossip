
use nostr_proto::IdHex;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Reactions {
    pub upvotes: u64,
    pub downvotes: u64,
    pub emojis: Vec<(char, u64)>
}

impl Default for Reactions {
    fn default() -> Reactions {
        Reactions {
            upvotes: 0,
            downvotes: 0,
            emojis: Vec::new()
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EventMetadata {
    pub id: IdHex,
    pub replies: Vec<IdHex>,
    pub in_reply_to: Option<IdHex>,
    pub reactions: Reactions,
    pub deleted_reason: Option<String>,
    pub client: Option<String>,
    pub hashtags: Vec<String>,
    pub subject: Option<String>,
    pub urls: Vec<String>,
}

impl EventMetadata {
    pub fn new(id: IdHex) -> EventMetadata {
        EventMetadata {
            id: id,
            replies: Vec::new(),
            in_reply_to: None,
            reactions: Default::default(),
            deleted_reason: None,
            client: None,
            hashtags: Vec::new(),
            subject: None,
            urls: Vec::new(),
        }
    }
}
