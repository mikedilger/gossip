use nostr_types::PublicKeyHex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbPerson {
    pub pubkey: PublicKeyHex,
    pub name: Option<String>,
    pub about: Option<String>,
    pub picture: Option<String>,
    pub dns_id: Option<String>,
    pub dns_id_valid: u8,
    pub dns_id_last_checked: Option<u64>,
    pub metadata_at: Option<i64>,
    pub followed: u8,
}
