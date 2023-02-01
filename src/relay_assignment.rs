use crate::db::{DbPersonRelay, DbRelay};
use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{PublicKeyHex, RelayUrl};
use std::collections::HashMap;
use std::fmt;

/// A RelayAssignment is a record of a relay which is serving (or will serve) the general
/// feed for a set of public keys.
pub struct RelayAssignment {
    pub relay: DbRelay,
    pub pubkeys: Vec<PublicKeyHex>,
}

/// Ways that the RelayPicker can fail
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum RelayPickerFailure {
    /// No people left to assign. A good result.
    NoPeopleLeft,

    /// No progress was made. A stuck result.
    NoProgress,

    /// No relays left to assign to. An unfortunate but best-we-can-do result.
    NoRelaysLeft,

    /// Caller specified a relay we do not have
    NoSuchRelayToTake,
}

impl fmt::Display for RelayPickerFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            RelayPickerFailure::NoPeopleLeft => write!(f, "All people accounted for."),
            RelayPickerFailure::NoProgress => write!(f, "Unable to make further progress."),
            RelayPickerFailure::NoRelaysLeft => write!(f, "No more relays to pick from"),
            RelayPickerFailure::NoSuchRelayToTake => write!(f, "No such relay to take"),
        }
    }
}

/// The RelayPicker is a structure that remembers which publickeys need relays, which
/// relays we aren't connected to, and person_relay_scores, and with this data can
/// assign public keys to relays.
#[derive(Debug, Default)]
pub struct RelayPicker {
    /// The relays to pick from.
    // Each time best() or take() is run, it returns a new RelayPicker
    // which removes that relay from this list
    pub relays: Vec<DbRelay>,

    /// The number of relays we should find for the given public key.
    // each run of best() decrements this as it assigns the public key
    // to a relay. This should start at settings.num_relays_per_person
    // for each person followed
    pub pubkey_counts: HashMap<PublicKeyHex, u8>,

    /// A ranking of relays per person.
    // best() and take() don't change this.
    pub person_relay_scores: Vec<(PublicKeyHex, RelayUrl, u64)>,
}

impl RelayPicker {
    /// This starts a new RelayPicker that has:
    ///  * All relays
    ///  * All followed public keys, with count starting at num_relays_per_person
    ///  * person relay scores for all person-relay pairings
    pub async fn new() -> Result<RelayPicker, Error> {
        // Load relays from the database
        let all_relays = DbRelay::fetch(None).await?;

        // Get all the people we follow
        let pubkeys: Vec<PublicKeyHex> = GLOBALS
            .people
            .get_followed_pubkeys()
            .iter()
            .map(|p| p.to_owned())
            .collect();

        let num_relays_per_person = GLOBALS.settings.read().await.num_relays_per_person;

        // Create pubkey counts for each person
        let mut pubkey_counts: HashMap<PublicKeyHex, u8> = HashMap::new();
        for pk in pubkeys.iter() {
            pubkey_counts.insert(pk.clone(), num_relays_per_person);
        }

        // Compute scores for each person_relay pairing
        let mut person_relay_scores: Vec<(PublicKeyHex, RelayUrl, u64)> = Vec::new();
        for pubkey in &pubkeys {
            let best_relays: Vec<(PublicKeyHex, RelayUrl, u64)> =
                DbPersonRelay::get_best_relays(pubkey.to_owned())
                    .await?
                    .iter()
                    .map(|(url, score)| (pubkey.to_owned(), url.to_owned(), *score))
                    .collect();
            person_relay_scores.extend(best_relays);
        }

        Ok(RelayPicker {
            relays: all_relays,
            pubkey_counts,
            person_relay_scores,
        })
    }

    pub async fn refresh_person_relay_scores(&mut self) -> Result<(), Error> {
        let pubkeys: Vec<PublicKeyHex> = self.pubkey_counts.keys().map(|k| k.to_owned()).collect();

        // Compute scores for each person_relay pairing
        let mut person_relay_scores: Vec<(PublicKeyHex, RelayUrl, u64)> = Vec::new();
        for pubkey in &pubkeys {
            let best_relays: Vec<(PublicKeyHex, RelayUrl, u64)> =
                DbPersonRelay::get_best_relays(pubkey.to_owned())
                    .await?
                    .iter()
                    .map(|(url, score)| (pubkey.to_owned(), url.to_owned(), *score))
                    .collect();
            person_relay_scores.extend(best_relays);
        }

        self.person_relay_scores = person_relay_scores;

        Ok(())
    }

    // FIXME - function to call when you start following someone

    // FIXME - function to call when you stop following someone

    // Place an assignment back into the relay picker. This should be called
    // when the assigned relay is no longer serving.
    #[allow(dead_code)]
    pub fn return_assignment(&mut self, assignment: RelayAssignment) {
        // The relay shouldn't be in our relays, but just in case
        if let Some(pos) = self
            .relays
            .iter()
            .position(|x| x.url == assignment.relay.url)
        {
            let _ = self.relays.swap_remove(pos);
        }

        // Put back the public keys
        for pubkey in assignment.pubkeys.iter() {
            self.pubkey_counts
                .entry(pubkey.to_owned())
                .and_modify(|e| *e += 1)
                .or_insert(1);
        }
    }

    /// Force RelayPicker to choose a specific relay next.
    #[allow(dead_code)]
    pub fn take(&mut self, relay_url: RelayUrl) -> Result<RelayAssignment, RelayPickerFailure> {
        if self.pubkey_counts.is_empty() {
            return Err(RelayPickerFailure::NoPeopleLeft);
        }
        if self.relays.is_empty() {
            return Err(RelayPickerFailure::NoRelaysLeft);
        }

        let winner_index = match self.relays.iter().position(|r| r.url == relay_url) {
            Some(pos) => pos,
            None => return Err(RelayPickerFailure::NoSuchRelayToTake),
        };

        self.consume(winner_index)
    }

    /// This function takes a RelayPicker which consists of a list of relays,
    /// a list of public keys, and a mapping between them.  It outputs a
    /// RelayAssignment structure which includes the best relay to listen to and
    /// the public keys such a relay will cover. It also outpus a new RelayPicker
    /// that contains only the remaining relays and public keys.
    pub fn pick(&mut self) -> Result<RelayAssignment, RelayPickerFailure> {
        if self.pubkey_counts.is_empty() {
            return Err(RelayPickerFailure::NoPeopleLeft);
        }
        if self.relays.is_empty() {
            return Err(RelayPickerFailure::NoRelaysLeft);
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
            let rank = (self.relays[i].rank as f32 * (1.3 * success_rate)) as u64;
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
    fn consume(&mut self, winner_index: usize) -> Result<RelayAssignment, RelayPickerFailure> {
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
            return Err(RelayPickerFailure::NoProgress);
        }

        // Remove entries with 0 more relays needed
        self.pubkey_counts.retain(|_, v| *v > 0);

        Ok(RelayAssignment {
            relay: winner,
            pubkeys: covered_public_keys,
        })
    }
}
