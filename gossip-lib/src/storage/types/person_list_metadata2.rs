use nostr_types::Unixtime;
use speedy::{Readable, Writable};

#[derive(Debug, Clone, PartialEq, Eq, Readable, Writable)]
pub struct PersonListMetadata2 {
    pub dtag: String,
    pub title: String,
    pub last_edit_time: Unixtime,
    pub event_created_at: Unixtime,
    pub event_public_len: usize,
    pub event_private_len: Option<usize>,
    pub favorite: bool,
    pub order: usize,
    pub private: bool,
}

impl Default for PersonListMetadata2 {
    fn default() -> PersonListMetadata2 {
        PersonListMetadata2 {
            dtag: "".to_owned(),
            title: "".to_owned(),
            last_edit_time: Unixtime::now().unwrap(),
            event_created_at: Unixtime(0),
            event_public_len: 0,
            event_private_len: None,
            favorite: false,
            order: 0,
            private: false,
        }
    }
}
