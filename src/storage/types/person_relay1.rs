use nostr_types::{PublicKey, RelayUrl, Unixtime};
use speedy::{Readable, Writable};

#[derive(Debug, Readable, Writable)]
pub struct PersonRelay1 {
    // The person
    pub pubkey: PublicKey,

    // The relay associated with that person
    pub url: RelayUrl,

    // The last time we fetched one of the person's events from this relay
    pub last_fetched: Option<u64>,

    // When we follow someone at a relay
    pub last_suggested_kind3: Option<u64>,

    // When we get their nip05 and it specifies this relay
    pub last_suggested_nip05: Option<u64>,

    // Updated when a 'p' tag on any event associates this person and relay via the
    // recommended_relay_url field
    pub last_suggested_bytag: Option<u64>,

    pub read: bool,

    pub write: bool,

    // When we follow someone at a relay, this is set true
    pub manually_paired_read: bool,

    // When we follow someone at a relay, this is set true
    pub manually_paired_write: bool,
}

impl PersonRelay1 {
    pub fn new(pubkey: PublicKey, url: RelayUrl) -> PersonRelay1 {
        PersonRelay1 {
            pubkey,
            url,
            last_fetched: None,
            last_suggested_kind3: None,
            last_suggested_nip05: None,
            last_suggested_bytag: None,
            read: false,
            write: false,
            manually_paired_read: false,
            manually_paired_write: false,
        }
    }

    // This ranks the relays that a person writes to, but does not consider local
    // factors such as our relay rank or the success rate of the relay.
    pub fn write_rank(mut dbprs: Vec<PersonRelay1>) -> Vec<(RelayUrl, u64)> {
        let now = Unixtime::now().unwrap().0 as u64;
        let mut output: Vec<(RelayUrl, u64)> = Vec::new();

        let scorefn = |when: u64, fade_period: u64, base: u64| -> u64 {
            let dur = now.saturating_sub(when); // seconds since
            let periods = (dur / fade_period) + 1; // minimum one period
            base / periods
        };

        for dbpr in dbprs.drain(..) {
            let mut score = 0;

            // 'write' is an author-signed explicit claim of where they write
            if dbpr.write || dbpr.manually_paired_write {
                score += 20;
            }

            // kind3 is our memory of where we are following someone
            if let Some(when) = dbpr.last_suggested_kind3 {
                score += scorefn(when, 60 * 60 * 24 * 30, 7);
            }

            // nip05 is an unsigned dns-based author claim of using this relay
            if let Some(when) = dbpr.last_suggested_nip05 {
                score += scorefn(when, 60 * 60 * 24 * 15, 4);
            }

            // last_fetched is gossip verified happened-to-work-before
            if let Some(when) = dbpr.last_fetched {
                score += scorefn(when, 60 * 60 * 24 * 3, 3);
            }

            // last_suggested_bytag is an anybody-signed suggestion
            if let Some(when) = dbpr.last_suggested_bytag {
                score += scorefn(when, 60 * 60 * 24 * 2, 1);
            }

            // Prune score=0 associations
            if score == 0 {
                continue;
            }

            output.push((dbpr.url, score));
        }

        output.sort_by(|(_, score1), (_, score2)| score2.cmp(score1));

        // prune everything below a score of 20, but only after the first 6 entries
        while output.len() > 6 && output[output.len() - 1].1 < 20 {
            let _ = output.pop();
        }

        output
    }

    // This ranks the relays that a person reads from, but does not consider local
    // factors such as our relay rank or the success rate of the relay.
    pub fn read_rank(mut dbprs: Vec<PersonRelay1>) -> Vec<(RelayUrl, u64)> {
        let now = Unixtime::now().unwrap().0 as u64;
        let mut output: Vec<(RelayUrl, u64)> = Vec::new();

        let scorefn = |when: u64, fade_period: u64, base: u64| -> u64 {
            let dur = now.saturating_sub(when); // seconds since
            let periods = (dur / fade_period) + 1; // minimum one period
            base / periods
        };

        for dbpr in dbprs.drain(..) {
            let mut score = 0;

            // 'read' is an author-signed explicit claim of where they read
            if dbpr.read || dbpr.manually_paired_read {
                score += 20;
            }

            // kind3 is our memory of where we are following someone
            if let Some(when) = dbpr.last_suggested_kind3 {
                score += scorefn(when, 60 * 60 * 24 * 30, 7);
            }

            // nip05 is an unsigned dns-based author claim of using this relay
            if let Some(when) = dbpr.last_suggested_nip05 {
                score += scorefn(when, 60 * 60 * 24 * 15, 4);
            }

            // last_fetched is gossip verified happened-to-work-before
            if let Some(when) = dbpr.last_fetched {
                score += scorefn(when, 60 * 60 * 24 * 3, 3);
            }

            // last_suggested_bytag is an anybody-signed suggestion
            if let Some(when) = dbpr.last_suggested_bytag {
                score += scorefn(when, 60 * 60 * 24 * 2, 1);
            }

            // Prune score=0 associations
            if score == 0 {
                continue;
            }

            output.push((dbpr.url, score));
        }

        output.sort_by(|(_, score1), (_, score2)| score2.cmp(score1));

        // prune everything below a score 20, but only after the first 6 entries
        while output.len() > 6 && output[output.len() - 1].1 < 20 {
            let _ = output.pop();
        }
        output
    }
}
