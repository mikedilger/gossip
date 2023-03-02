use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{PublicKey, Tag};
use parking_lot::RwLock;

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
            return serde_json::to_string(&tag).unwrap_or_default();
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

    pub fn set(&self, tag_str: &str) -> Result<(), Error> {
        if tag_str.is_empty() {
            *self.delegatee_tag.write() = None;
        } else {
            let tag = serde_json::from_str(tag_str)?;
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
        self.set(&GLOBALS.settings.read().delegatee_tag)
    }

    pub async fn save_through_settings(&self) -> Result<(), Error> {
        GLOBALS.settings.write().delegatee_tag = self.get_delegatee_tag_as_str();
        let settings = GLOBALS.settings.read().clone();
        settings.save().await?;
        Ok(())
    }
}
