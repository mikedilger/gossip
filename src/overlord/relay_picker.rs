use crate::db::{DbPersonRelay, DbRelay};
use crate::error::Error;
use nostr_proto::PublicKeyHex;
use tracing::info;

/// See RelayPicker::best()
#[allow(dead_code)]
pub struct RelayPicker {
    pub relays: Vec<DbRelay>,
    pub pubkeys: Vec<PublicKeyHex>,
    pub person_relays: Vec<DbPersonRelay>,
}

impl RelayPicker {
    #[allow(dead_code)]
    pub fn is_degenerate(&self) -> bool {
        self.relays.is_empty() || self.pubkeys.is_empty() || self.person_relays.is_empty()
    }

    /// This function takes a RelayPicker which consists of a list of relays,
    /// a list of public keys, and a mapping between them.  It outputs a
    /// BestRelay structure which includes the best relay to listen to and
    /// the public keys such a relay will cover. It also outpus a new RelayPicker
    /// that contains only the remaining relays and public keys.
    #[allow(dead_code)]
    pub fn best(mut self) -> Result<(BestRelay, RelayPicker), Error> {
        if self.pubkeys.is_empty() {
            return Err(Error::General(
                "best_relay called for zero people".to_owned(),
            ));
        }
        if self.relays.is_empty() {
            return Err(Error::General(
                "best_relay called for zero relays".to_owned(),
            ));
        }

        info!(
            "Searching for the best relay among {} for {} people",
            self.relays.len(),
            self.pubkeys.len()
        );

        // Keep score
        let mut score: Vec<u64> = [0].repeat(self.relays.len());

        // Count how many keys a relay covers, to use as part of it's score
        for person_relay in self.person_relays.iter() {
            let i = match self
                .relays
                .iter()
                .position(|relay| relay.url == person_relay.relay)
            {
                Some(index) => index,
                None => continue, // we don't have that relay?
            };
            score[i] += 1;
        }

        // Multiply scores by relay rank
        #[allow(clippy::needless_range_loop)]
        for i in 0..self.relays.len() {
            score[i] *= self.relays[i].rank.unwrap_or(1);
        }

        let winner_index = score
            .iter()
            .enumerate()
            .max_by(|x: &(usize, &u64), y: &(usize, &u64)| x.1.cmp(y.1))
            .unwrap()
            .0;

        let winner = self.relays.swap_remove(winner_index);

        let covered_public_keys: Vec<PublicKeyHex> = self
            .person_relays
            .iter()
            .filter(|x| x.relay == winner.url)
            .map(|x| PublicKeyHex(x.person.clone()))
            .collect();

        self.pubkeys.retain(|x| !covered_public_keys.contains(x));

        self.person_relays.retain(|pr| {
            !covered_public_keys.contains(&PublicKeyHex(pr.person.clone()))
                && pr.relay != winner.url
        });

        Ok((
            BestRelay {
                relay: winner,
                pubkeys: covered_public_keys,
            },
            self,
        ))
    }
}

/// See RelayPicker::best()
pub struct BestRelay {
    pub relay: DbRelay,
    pub pubkeys: Vec<PublicKeyHex>,
}

impl BestRelay {
    #[allow(dead_code)]
    pub fn is_degenerate(&self) -> bool {
        self.pubkeys.is_empty() || self.relay.rank == Some(0)
    }
}
