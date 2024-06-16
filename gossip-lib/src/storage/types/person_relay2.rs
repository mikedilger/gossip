use nostr_types::{PublicKey, RelayUrl, Unixtime};
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

    pub fn association_rank(&self, now: Unixtime, write: bool) -> u64 {
        let now = now.0 as u64;
        let scorefn = |when: u64, fade_period: u64, base: u64| -> u64 {
            let dur = now.saturating_sub(when); // seconds since
            base * fade_period / fade_period.max(dur)
        };

        let mut score = 0;

        if write {
            if self.write {
                // 'write' is an author-signed explicit claim of where they write
                score += 20;
            }
        } else {
            if self.read {
                // 'read' is an author-signed explicit claim of where they read
                score += 20;
            }
        }

        // last_fetched is gossip verified happened-to-work-before
        if let Some(when) = self.last_fetched {
            score += scorefn(when, 60 * 60 * 24 * 3, 4);
        }

        // last_suggested is an anybody-signed suggestion
        if let Some(when) = self.last_suggested {
            score += scorefn(when, 60 * 60 * 24 * 2, 1);
        }

        score
    }
}
