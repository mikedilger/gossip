use crate::{
    ContentEncryptionAlgorithm, Error, Event, EventKind, ParsedTag, PreEvent, PublicKey, Signer,
    Unixtime,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A NIP-46 request, found stringified in the content of a kind 24133 event
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Nip46Request {
    /// The Request ID
    pub id: String,

    /// The Request Method (See NIP-46)
    pub method: String,

    /// The Request parameters
    pub params: Vec<String>,
}

impl Nip46Request {
    /// Create a new request object
    pub fn new(method: String, params: Vec<String>) -> Nip46Request {
        Nip46Request {
            id: textnonce::TextNonce::new().into_string(),
            method,
            params,
        }
    }

    /// Create a NIP-46 request event from this request
    pub async fn to_event(
        &self,
        bunker_pubkey: PublicKey,
        signer: Arc<dyn Signer>,
    ) -> Result<Event, Error> {
        let request_string = serde_json::to_string(self)?;

        let content = signer
            .encrypt(
                &bunker_pubkey,
                request_string.as_str(),
                ContentEncryptionAlgorithm::Nip44v2,
            )
            .await?;

        let pre_event = PreEvent {
            pubkey: signer.public_key(),
            created_at: Unixtime::now(),
            kind: EventKind::NostrConnect,
            tags: vec![ParsedTag::Pubkey {
                pubkey: bunker_pubkey,
                recommended_relay_url: None,
                petname: None,
            }
            .into_tag()],
            content,
        };

        let event = signer.sign_event(pre_event).await?;

        Ok(event)
    }
}
