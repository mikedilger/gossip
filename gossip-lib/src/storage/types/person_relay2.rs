use nostr_types::{PublicKey, RelayUrl, RelayUsage, Unixtime};
use serde::{Deserialize, Serialize};
use speedy::{Readable, Writable};

// THIS IS HISTORICAL FOR MIGRATIONS AND THE STRUCTURES SHOULD NOT BE EDITED

/// A person-relay association
#[derive(Debug, Readable, Writable, Serialize, Deserialize)]
pub struct PersonRelay2 {
    /// The person
    pub pubkey: PublicKey,

    /// The relay associated with that person
    pub url: RelayUrl,

    /// If they set 'read' on their relay list (kind 10002 or kind 3 contents)
    /// or nip05 relays (which sets both read and write)
    pub read: bool,

    /// If they set 'write' on their relay list (kind 10002 or kind 3 contents)
    /// or nip05 relays (which sets both read and write)
    pub write: bool,

    /// If it was listed in their kind-10050 NIP-17 DM relay list
    pub dm: bool,

    /// The last time we fetched one of the person's events from this relay
    pub last_fetched: Option<u64>,

    /// The last time it was suggested by a 3rd party
    /// (e.g. in a 'p' tag recommended_relay_url)
    pub last_suggested: Option<u64>,
}

impl PersonRelay2 {
    pub fn new(pubkey: PublicKey, url: RelayUrl) -> PersonRelay2 {
        PersonRelay2 {
            pubkey,
            url,
            read: false,
            write: false,
            dm: false,
            last_fetched: None,
            last_suggested: None,
        }
    }

    // 1.0 means it is in their relay list
    // 0.2 (with halflife of 14 days) if we found their events there recently
    // 0.1 (with halflife of 7 days) if a relay hint suggested it
    pub fn association_score(&self, now: Unixtime, usage: RelayUsage) -> f32 {
        let now = now.0 as u64;

        let mut score = 0.0;

        if usage == RelayUsage::Outbox {
            if self.write {
                // 'write' is an author-signed explicit claim of where they write
                score += 1.0;
            }
        } else if usage == RelayUsage::Inbox {
            if self.read {
                // 'read' is an author-signed explicit claim of where they read
                score += 1.0;
            }
        }

        // last_fetched is gossip verified happened-to-work-before
        if let Some(when) = self.last_fetched {
            let base = 0.2_f32;
            let halflife_seconds = 60 * 60 * 24 * 14;
            let elapsed_seconds = now.saturating_sub(when);
            let delta = crate::misc::exponential_decay(base, halflife_seconds, elapsed_seconds);
            score += delta;
        }

        // last_suggested is an anybody-signed suggestion
        if let Some(when) = self.last_suggested {
            let base = 0.1_f32;
            let halflife_seconds = 60 * 60 * 24 * 7;
            let elapsed_seconds = now.saturating_sub(when);
            let delta = crate::misc::exponential_decay(base, halflife_seconds, elapsed_seconds);
            score += delta;
        }

        score
    }
}
