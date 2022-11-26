
use crate::db::DbEvent;
use nostr_proto::Event;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct JsEvent {
    pub id: String,
    pub pubkey: String,
    pub created_at: i64,
    pub kind: u64,
    pub content: String
}

impl From<Event> for JsEvent {
    fn from(e: Event) -> JsEvent {
        JsEvent {
            id: e.id.as_hex_string(),
            pubkey: e.pubkey.as_hex_string(),
            created_at: e.created_at.0,
            kind: e.kind.into(),
            content: e.content,
        }
    }
}

impl From<&Event> for JsEvent {
    fn from(e: &Event) -> JsEvent {
        JsEvent {
            id: e.id.as_hex_string(),
            pubkey: e.pubkey.as_hex_string(),
            created_at: e.created_at.0,
            kind: e.kind.into(),
            content: e.content.clone(),
        }
    }
}

impl From<DbEvent> for JsEvent {
    fn from(e: DbEvent) -> JsEvent {
        JsEvent {
            id: e.id.0,
            pubkey: e.pubkey.0,
            created_at: e.created_at,
            kind: e.kind,
            content: e.content
        }
    }
}

impl From<&DbEvent> for JsEvent {
    fn from(e: &DbEvent) -> JsEvent {
        JsEvent {
            id: e.id.0.clone(),
            pubkey: e.pubkey.0.clone(),
            created_at: e.created_at,
            kind: e.kind,
            content: e.content.clone()
        }
    }
}
