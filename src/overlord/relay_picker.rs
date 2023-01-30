use crate::db::DbRelay;
use crate::error::Error;
use nostr_types::{PublicKeyHex, RelayUrl};
use std::collections::HashMap;

/// See RelayPicker::best()
pub struct RelayPicker {
    /// The relays to pick from.
    // Each time best() is run, it returns a new RelayPicker
    // which removes that relay from this list
    pub relays: Vec<DbRelay>,

    /// The number of relays we should find for the given public key.
    // each run of best() decrements this as it assigns the public key
    // to a relay. This should start at settings.num_relays_per_person
    // for each person followed
    pub pubkey_counts: HashMap<PublicKeyHex, u8>,

    /// A ranking of relays per person.
    // best() doesn't change this.
    pub person_relay_scores: Vec<(PublicKeyHex, RelayUrl, u64)>,
}

impl RelayPicker {
    pub fn is_degenerate(&self) -> bool {
        self.relays.is_empty()
            || self.pubkey_counts.is_empty()
            || self.person_relay_scores.is_empty()
    }

    /// Force RelayPicker to choose a specific relay next. This is useful when
    /// we are already connected to some relays and need to pick more.
    #[allow(dead_code)]
    pub fn take(self, relay_url: RelayUrl) -> Result<(BestRelay, RelayPicker), Error> {
        if self.pubkey_counts.is_empty() {
            return Err(Error::General(
                "RelayPicker::take() called for zero people".to_owned(),
            ));
        }
        if self.relays.is_empty() {
            return Err(Error::General(
                "RelayPicker::take() called for zero relays".to_owned(),
            ));
        }

        let winner_index = match self.relays.iter().position(|r| r.url == relay_url) {
            Some(pos) => pos,
            None => {
                return Err(Error::General(
                    "RelayPicker::take() called with relay that is not present".to_owned(),
                ))
            }
        };

        self.consume(winner_index)
    }

    /// This function takes a RelayPicker which consists of a list of relays,
    /// a list of public keys, and a mapping between them.  It outputs a
    /// BestRelay structure which includes the best relay to listen to and
    /// the public keys such a relay will cover. It also outpus a new RelayPicker
    /// that contains only the remaining relays and public keys.
    pub fn best(self) -> Result<(BestRelay, RelayPicker), Error> {
        if self.pubkey_counts.is_empty() {
            return Err(Error::General(
                "RelayPicker::best() called for zero people".to_owned(),
            ));
        }
        if self.relays.is_empty() {
            return Err(Error::General(
                "RelayPicker::best() called for zero relays".to_owned(),
            ));
        }

        tracing::info!(
            "Searching for the best relay among {} for {} people",
            self.relays.len(),
            self.pubkey_counts.len()
        );

        // Keep score
        let mut scoreboard: Vec<u64> = [0].repeat(self.relays.len());

        // Assign scores to relays
        for (pubkeyhex, url, score) in self.person_relay_scores.iter() {
            // Skip if person is already well covered
            if let Some(pkc) = self.pubkey_counts.get(pubkeyhex) {
                if *pkc == 0 {
                    // person is already covered by enough relays
                    continue;
                }
            } else {
                continue; // person is not relevant.
            }

            // Get the index
            let i = match self.relays.iter().position(|relay| relay.url == *url) {
                Some(index) => index,
                None => continue, // That relay is not a contender
            };

            scoreboard[i] += score;
        }

        // Multiply scores by relay rank
        #[allow(clippy::needless_range_loop)]
        for i in 0..self.relays.len() {
            // Here we compute a relay rank based on .rank
            // but also on success rate
            let success_rate: f32 = self.relays[i].success_rate();
            let rank = (self.relays[i].rank.unwrap_or(3) as f32 * (1.3 * success_rate)) as u64;
            scoreboard[i] *= rank;
        }

        let winner_index = scoreboard
            .iter()
            .enumerate()
            .max_by(|x: &(usize, &u64), y: &(usize, &u64)| x.1.cmp(y.1))
            .unwrap()
            .0;

        self.consume(winner_index)
    }

    // This is the bottom-half of the code for both best() and take()
    fn consume(mut self, winner_index: usize) -> Result<(BestRelay, RelayPicker), Error> {
        let winner = self.relays.swap_remove(winner_index);

        let covered_public_keys: Vec<PublicKeyHex> = self
            .person_relay_scores
            .iter()
            .filter(|(_, url, score)| *url == winner.url && *score > 0)
            .map(|(pkh, _, _)| pkh.to_owned())
            .collect();

        // Decrement entries where we the winner covers them
        let mut changed = false;
        for (pubkey, count) in self.pubkey_counts.iter_mut() {
            if covered_public_keys.contains(pubkey) {
                *count -= 1;
                changed = true;
            }
        }

        // If the pubkey_counts did not change
        if !changed {
            // then we are now degenerate.
            // Output a BestRelay with zero public keys to signal this
            return Ok((
                BestRelay {
                    relay: winner,
                    pubkeys: vec![],
                },
                self,
            ));
        }

        // Remove entries with 0 more relays needed
        self.pubkey_counts.retain(|_, v| *v > 0);

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
    pub fn is_degenerate(&self) -> bool {
        self.pubkeys.is_empty() || self.relay.rank == Some(0)
    }
}
