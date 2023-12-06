use nostr_types::Unixtime;
use speedy::{Readable, Writable};

#[derive(Debug, Clone, PartialEq, Eq, Readable, Writable)]
pub struct PersonListMetadata1 {
    pub dtag: String,
    pub name: String,
    pub last_edit_time: Unixtime,
    pub event_created_at: Unixtime,
    pub event_public_len: usize,
    pub event_private_len: Option<usize>,
}
