use nostr_types::{PublicKey, Unixtime};
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
        let mut output = String::new();
        let mut first = true;
        for pubkey in &self.0 {
            if first {
                first = false;
            } else {
                output.push_str(", ");
            }

            let name = crate::names::display_name_from_pubkey_lookup(pubkey);
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
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DmChannelData {
    pub dm_channel: DmChannel,
    pub latest_message: Unixtime,
    pub message_count: usize,
    pub unread_message_count: usize,
}
