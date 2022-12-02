
use crate::db::DbEvent;
use nostr_proto::{Event, IdHex, PublicKeyHex};
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
pub struct JsEvent {
    pub id: IdHex,
    pub pubkey: Option<String>,
    pub created_at: Option<i64>,
    pub kind: Option<u64>,
    pub content: Option<String>,
    pub replies: Vec<IdHex>,
    pub in_reply_to: Option<IdHex>,
    pub reactions: Reactions,
    pub deleted_reason: Option<String>,
    pub client: Option<String>,
    pub hashtags: Vec<String>,
    pub subject: Option<String>,
    pub urls: Vec<String>,
    pub last_reply_at: Option<i64>,
}

impl JsEvent {
    pub fn new(id: IdHex) -> JsEvent {
        JsEvent {
            id: id,
            pubkey: None,
            created_at: None,
            kind: None,
            content: None,
            replies: Vec::new(),
            in_reply_to: None,
            reactions: Default::default(),
            deleted_reason: None,
            client: None,
            hashtags: Vec::new(),
            subject: None,
            urls: Vec::new(),
            last_reply_at: None,
        }
    }

    // Sometimes we start a JsEvent from new() because some other
    // event wants to set some data about it before we have the
    // actual main event part.  This is so we can set the actual
    // main event part without erasing the metadata part
    pub fn set_main_event_data(&mut self, event: JsEvent) {
        self.id = event.id;
        self.pubkey = event.pubkey;
        self.created_at = event.created_at;
        self.kind = event.kind;
        self.content = event.content;

        self.last_reply_at = event.created_at;
    }
}

impl From<&Event> for JsEvent {
    fn from(event: &Event) -> JsEvent {
        JsEvent {
            id: From::from(event.id),
            pubkey: Some(PublicKeyHex::from(event.pubkey).0),
            created_at: Some(event.created_at.0),
            kind: Some(u64::from(event.kind)),
            content: Some(event.content.clone()),
            replies: Vec::new(),
            in_reply_to: None,
            reactions: Default::default(),
            deleted_reason: None,
            client: None,
            hashtags: Vec::new(),
            subject: None,
            urls: Vec::new(),
            last_reply_at: Some(event.created_at.0),
        }
    }
}

impl From<&DbEvent> for JsEvent {
    fn from(dbevent: &DbEvent) -> JsEvent {
        JsEvent {
            id: dbevent.id.clone(),
            pubkey: Some(dbevent.pubkey.0.clone()),
            created_at: Some(dbevent.created_at),
            kind: Some(dbevent.kind),
            content: Some(dbevent.content.clone()),
            replies: Vec::new(),
            in_reply_to: None,
            reactions: Default::default(),
            deleted_reason: None,
            client: None,
            hashtags: Vec::new(),
            subject: None,
            urls: Vec::new(),
            last_reply_at: Some(dbevent.created_at),
        }
    }
}
