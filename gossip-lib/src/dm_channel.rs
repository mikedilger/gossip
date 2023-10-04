use crate::globals::GLOBALS;
use nostr_types::{Event, EventKind, PublicKey, Unixtime};
use sha2::Digest;

/// This represents a DM (direct message) channel which includes a set
/// of participants (usually just one, but can be a small group).
// internally the pubkeys are kept sorted so they can be compared
// that is why we don't expose the inner field directly.
//
// The pubkey of the gossip user is not included. If they send themselves
// a note, that channel has an empty vec.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DmChannel(Vec<PublicKey>);

impl DmChannel {
    pub fn new(public_keys: &[PublicKey]) -> DmChannel {
        let mut vec = public_keys.to_owned();
        vec.sort();
        vec.dedup();
        DmChannel(vec)
    }

    pub fn keys(&self) -> &[PublicKey] {
        &self.0
    }

    pub fn name(&self) -> String {
        if self.0.is_empty() {
            return match GLOBALS.signer.public_key() {
                Some(pk) => crate::names::tag_name_from_pubkey_lookup(&pk),
                None => "[NOBODY]".to_string(),
            };
        }

        let mut output = String::new();
        let mut first = true;
        for pubkey in &self.0 {
            if first {
                first = false;
            } else {
                output.push_str(", ");
            }

            let name = crate::names::tag_name_from_pubkey_lookup(pubkey);
            output.push_str(&name);
        }
        output
    }

    pub fn unique_id(&self) -> String {
        let mut hasher = sha2::Sha256::new();
        for pk in &self.0 {
            hasher.update(pk.as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    pub fn from_event(event: &Event, my_pubkey: Option<PublicKey>) -> Option<DmChannel> {
        let my_pubkey = match my_pubkey {
            Some(pk) => pk,
            None => match GLOBALS.signer.public_key() {
                Some(pk) => pk,
                None => return None,
            },
        };

        if event.kind == EventKind::EncryptedDirectMessage {
            let mut people: Vec<PublicKey> = event
                .people()
                .iter()
                .filter_map(|(pk, _, _)| PublicKey::try_from(pk).ok())
                .collect();
            people.push(event.pubkey);
            people.retain(|p| *p != my_pubkey);
            if people.len() > 1 {
                None
            } else {
                Some(Self::new(&people))
            }
        } else if event.kind == EventKind::GiftWrap {
            if let Ok(rumor) = GLOBALS.signer.unwrap_giftwrap(event) {
                let rumor_event = rumor.into_event_with_bad_signature();
                let mut people: Vec<PublicKey> = rumor_event
                    .people()
                    .iter()
                    .filter_map(|(pk, _, _)| PublicKey::try_from(pk).ok())
                    .collect();
                people.push(rumor_event.pubkey); // include author too
                people.retain(|p| *p != my_pubkey);
                Some(Self::new(&people))
            } else {
                None
            }
        } else if event.kind == EventKind::DmChat {
            // unwrapped rumor
            let mut people: Vec<PublicKey> = event
                .people()
                .iter()
                .filter_map(|(pk, _, _)| PublicKey::try_from(pk).ok())
                .collect();
            people.push(event.pubkey); // include author too
            people.retain(|p| *p != my_pubkey);
            Some(Self::new(&people))
        } else {
            None
        }
    }
}

/// Data about a DM channel such as when the latest message occured, how many massages
/// it has, and how many are unread.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DmChannelData {
    pub dm_channel: DmChannel,
    pub latest_message: Unixtime,
    pub message_count: usize,
    pub unread_message_count: usize,
}
