use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{PublicKey, PublicKeyHex, Signature, SignatureHex, Tag};
use parking_lot::RwLock;
use serde_json::json;

#[derive(Default)]
pub struct Delegation {
    // Delegatee NIP-26 delegation tag, optional
    delegatee_tag: RwLock<Option<Tag>>,
}

impl Delegation {
    pub fn get_delegatee_tag(&self) -> Option<Tag> {
        self.delegatee_tag.read().clone()
    }

    pub fn get_delegatee_tag_as_str(&self) -> String {
        if let Some(tag) = self.get_delegatee_tag() {
            return serialize_delegation_tag(&tag);
        }
        String::new()
    }

    pub fn get_delegator_pubkey(&self) -> Option<PublicKey> {
        if let Some(tag) = self.get_delegatee_tag() {
            if let Tag::Delegation {
                pubkey,
                conditions: _,
                sig: _,
            } = tag
            {
                if let Ok(pk) = PublicKey::try_from_hex_string(pubkey.as_str()) {
                    return Some(pk);
                }
            }
        }
        None
    }

    pub fn get_delegator_pubkey_as_bech32_str(&self) -> Option<String> {
        if let Some(pubkey) = self.get_delegator_pubkey() {
            Some(pubkey.try_as_bech32_string().unwrap_or_default())
        } else {
            None
        }
    }

    pub fn set(&self, tag_str: &str) -> Result<(), String> {
        if tag_str.is_empty() {
            *self.delegatee_tag.write() = None;
        } else {
            let (tag, _pubkey) = parse_delegation_tag(tag_str)?;
            *self.delegatee_tag.write() = Some(tag);
        }
        Ok(())
    }

    pub fn reset(&self) -> bool {
        if let Some(_tag) = self.get_delegatee_tag() {
            *self.delegatee_tag.write() = None;
            true
        } else {
            false
        }
    }

    pub fn load_through_settings(&self) -> Result<(), Error> {
        self.set(&GLOBALS.settings.read().delegatee_tag).map_err(|e| Error::Delegation(e))
    }

    pub async fn save_through_settings(&self) -> Result<(), Error> {
        GLOBALS.settings.write().delegatee_tag = self.get_delegatee_tag_as_str();
        let settings = GLOBALS.settings.read().clone();
        settings.save().await?;
        Ok(())
    }
}

// Returns parsed tag & delegator pub; error is string (for simplicity)
// TODO should come from nostr-types
pub(crate) fn parse_delegation_tag(tag: &str) -> Result<(Tag, PublicKey), String> {
    // TODO parsing should be done using nostr crate v0.19 DelegationTag
    match json::parse(tag) {
        Err(e) => return Err(format!("Could not parse tag, {}", e.to_string())),
        Ok(jv) => {
            if !jv.is_array() || jv.len() < 4 {
                return Err(format!("Expected array with 4 elements"));
            }
            if !jv[0].is_string() || !jv[1].is_string() || !jv[2].is_string() || !jv[3].is_string()
            {
                return Err(format!("Expected array with 4 strings"));
            }
            if jv[0].as_str().unwrap() != "delegation" {
                return Err(format!("First string should be 'delegation'"));
            }
            match PublicKey::try_from_hex_string(jv[1].as_str().unwrap()) {
                Err(e) => return Err(format!("Could not parse public key, {}", e.to_string())),
                Ok(public_key) => {
                    let pubkey = PublicKeyHex::from(public_key);
                    let conditions = jv[2].as_str().unwrap().to_string();
                    let sig_str = jv[3].as_str().unwrap();
                    match Signature::try_from_hex_string(sig_str) {
                        Err(e) => {
                            return Err(format!("Could not parse signature, {}", e.to_string()))
                        }
                        Ok(signature) => {
                            let sig = SignatureHex::from(signature);
                            Ok((
                                Tag::Delegation {
                                    pubkey,
                                    conditions,
                                    sig,
                                },
                                public_key,
                            ))
                        }
                    }
                }
            }
        }
    }
}

/// Serialize a delegation tag into JSON string
// TODO should come from nostr-types
pub(crate) fn serialize_delegation_tag(tag: &Tag) -> String {
    match tag {
        Tag::Delegation {
            pubkey,
            conditions,
            sig,
        } => json!(["delegation", pubkey.as_str(), conditions, sig.to_string(),]).to_string(),
        _ => "".to_string(),
    }
}
