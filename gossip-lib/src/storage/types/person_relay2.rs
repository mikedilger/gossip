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

    // This ranks the relays that a person writes to, but does not consider local
    // factors such as our relay rank or the success rate of the relay.
    pub fn write_rank(mut dbprs: Vec<PersonRelay2>) -> Vec<(RelayUrl, u64)> {
        let now = Unixtime::now().unwrap().0 as u64;
        let mut output: Vec<(RelayUrl, u64)> = Vec::new();

        let scorefn = |when: u64, fade_period: u64, base: u64| -> u64 {
            let dur = now.saturating_sub(when); // seconds since
            base * fade_period / fade_period.max(dur)
        };

        for dbpr in dbprs.drain(..) {
            let mut score = 0;

            // 'write' is an author-signed explicit claim of where they write
            if dbpr.write {
                score += 20;
            }

            // last_fetched is gossip verified happened-to-work-before
            if let Some(when) = dbpr.last_fetched {
                score += scorefn(when, 60 * 60 * 24 * 3, 4);
            }

            // last_suggested is an anybody-signed suggestion
            if let Some(when) = dbpr.last_suggested {
                score += scorefn(when, 60 * 60 * 24 * 2, 1);
            }

            // Prune score=0 associations
            if score == 0 {
                continue;
            }

            output.push((dbpr.url, score));
        }

        output.sort_by(|(_, score1), (_, score2)| score2.cmp(score1));

        output
    }

    // This ranks the relays that a person reads from, but does not consider local
    // factors such as our relay rank or the success rate of the relay.
    pub fn read_rank(mut dbprs: Vec<PersonRelay2>) -> Vec<(RelayUrl, u64)> {
        let now = Unixtime::now().unwrap().0 as u64;
        let mut output: Vec<(RelayUrl, u64)> = Vec::new();

        let scorefn = |when: u64, fade_period: u64, base: u64| -> u64 {
            let dur = now.saturating_sub(when); // seconds since
            base * fade_period / fade_period.max(dur)
        };

        for dbpr in dbprs.drain(..) {
            let mut score = 0;

            // 'read' is an author-signed explicit claim of where they read
            if dbpr.read {
                score += 20;
            }

            // last_fetched is gossip verified happened-to-work-before
            if let Some(when) = dbpr.last_fetched {
                score += scorefn(when, 60 * 60 * 24 * 3, 4);
            }

            // last_suggested is an anybody-signed suggestion
            if let Some(when) = dbpr.last_suggested {
                score += scorefn(when, 60 * 60 * 24 * 2, 1);
            }

            // Prune score=0 associations
            if score == 0 {
                continue;
            }

            output.push((dbpr.url, score));
        }

        output.sort_by(|(_, score1), (_, score2)| score2.cmp(score1));

        output
    }
}
