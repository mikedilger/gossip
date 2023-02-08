use crate::db::{DbPersonRelay, DbRelay, Direction};
use crate::error::Error;
use crate::globals::GLOBALS;
use dashmap::{DashMap, DashSet};
use nostr_types::{PublicKeyHex, RelayUrl, Unixtime};

/// A RelayAssignment2 is a record of a relay which is serving (or will serve) the general
/// feed for a set of public keys.
#[derive(Debug, Clone)]
pub struct RelayAssignment2 {
    pub relay_url: RelayUrl,
    pub pubkeys: Vec<PublicKeyHex>,
}

impl RelayAssignment2 {
    pub fn merge_in(&mut self, other: RelayAssignment2) -> Result<(), Error> {
        if self.relay_url != other.relay_url {
            return Err(Error::General("Attempted to merge relay assignments on different relays".to_owned()));
        }
        self.pubkeys.extend(other.pubkeys);
        Ok(())
    }
}

// FIXME: move it into here
use crate::relay_picker::RelayPickerFailure;

/// The RelayPicker2 is a structure that helps assign people we follow to relays we watch.
/// It remembers which publickeys are assigned to which relays, which pubkeys need more
/// relays and how many, which relays need a time out, and person-relay scores for making
/// good assignments dynamically.
#[derive(Debug, Default)]
pub struct RelayPicker2 {
    /// All of the relays we might use
    pub all_relays: DashMap<RelayUrl, DbRelay>,

    /// All of the relays actually connected
    pub connected_relays: DashSet<RelayUrl>,

    /// All of the relays currently connected, with optional assignments.
    /// (Sometimes a relay is connected for a different kind of subscription.)
    pub relay_assignments: DashMap<RelayUrl, Option<RelayAssignment2>>,

    /// Relays which recently failed and which require a timeout before
    /// they can be chosen again.  The value is the time when it can be removed
    /// from this list.
    pub excluded_relays: DashMap<RelayUrl, i64>,

    /// For each followed pubkey that still needs assignments, the number of relay
    /// assignments it is seeking.  These start out at settings.num_relays_per_person
    /// (if the person doesn't have that many relays, it will do the best it can)
    pub pubkey_counts: DashMap<PublicKeyHex, u8>,

    /// A ranking of relays per person.
    pub person_relay_scores: DashMap<PublicKeyHex, Vec<(RelayUrl, u64)>>,
}

impl RelayPicker2 {
    /// This starts a new RelayPicker that has:
    ///  * All relays
    ///  * All followed public keys, with count starting at num_relays_per_person
    ///  * person relay scores for all person-relay pairings
    pub async fn init(&self) -> Result<(), Error> {
        // just in case it is re-initialized (not sure why it would be)
        self.all_relays.clear();
        self.connected_relays.clear();
        self.relay_assignments.clear();
        self.excluded_relays.clear();
        self.pubkey_counts.clear();
        self.person_relay_scores.clear();

        // Load relays from the database
        for (relay_url, dbrelay) in DbRelay::fetch(None)
            .await?
            .drain(..)
            .map(|dbr| (dbr.url.clone(), dbr))
        {
            self.all_relays.insert(relay_url, dbrelay);
        }

        self.refresh_person_relay_scores(true).await?;

        Ok(())
    }

    pub async fn refresh_person_relay_scores(&self, initialize_counts: bool) -> Result<(), Error> {
        self.person_relay_scores.clear();

        if initialize_counts {
            self.pubkey_counts.clear();
        }

        let num_relays_per_person = GLOBALS.settings.read().await.num_relays_per_person;

        // Get all the people we follow
        let pubkeys: Vec<PublicKeyHex> = GLOBALS
            .people
            .get_followed_pubkeys()
            .iter()
            .map(|p| p.to_owned())
            .collect();

        // Compute scores for each person_relay pairing
        for pubkey in &pubkeys {
            let best_relays: Vec<(RelayUrl, u64)> =
                DbPersonRelay::get_best_relays(pubkey.to_owned(), Direction::Write).await?;
            self.person_relay_scores.insert(pubkey.clone(), best_relays);

            if initialize_counts {
                self.pubkey_counts
                    .insert(pubkey.clone(), num_relays_per_person);
            }
        }

        Ok(())
    }

    /// When a relay disconnects, call this so that whatever assignments it might have
    /// had can be reassigned.  Then call pick_relays() again.
    pub fn relay_disconnected(&mut self, url: &RelayUrl) {
        // Remove from connected relays list
        if let Some((_key, maybe_assignment)) = self.relay_assignments.remove(url) {
            // Exclude the relay for the next 30 seconds
            let hence = Unixtime::now().unwrap().0 + 30;
            self.excluded_relays.insert(url.to_owned(), hence);
            tracing::debug!("{} goes into the penalty box until {}", url, hence,);

            // Take any assignment
            if let Some(relay_assignment) = maybe_assignment {
                // Put the public keys back into pubkey_counts
                for pubkey in relay_assignment.pubkeys.iter() {
                    self.pubkey_counts
                        .entry(pubkey.to_owned())
                        .and_modify(|e| *e += 1)
                        .or_insert(1);
                }
            }
        }
    }

    /// Create the next assignment, and return the RelayUrl that has it.
    /// The caller is responsible for making that assignment actually happen.
    pub fn pick(&self) -> Result<RelayUrl, RelayPickerFailure> {
        // Maybe include excluded relays
        let now = Unixtime::now().unwrap().0;
        self.excluded_relays.retain(|_, v| *v > now);

        if self.pubkey_counts.is_empty() {
            return Err(RelayPickerFailure::NoPeopleLeft);
        }

        // Keep score for each relay
        let scoreboard: DashMap<RelayUrl, u64> = self
            .all_relays
            .iter()
            .map(|x| (x.key().to_owned(), 0))
            .collect();

        // Assign scores to relays
        for elem in self.person_relay_scores.iter() {
            let pubkeyhex = elem.key();
            let relay_scores = elem.value();

            // Skip if this pubkey doesn't need any more assignments
            if let Some(pkc) = self.pubkey_counts.get(pubkeyhex) {
                if *pkc == 0 {
                    // person doesn't need anymore
                    continue;
                }
            } else {
                continue; // person doesn't need any
            }

            // Add scores of their relays
            for (relay, score) in relay_scores.iter() {
                // Skip relays that are excluded
                if self.excluded_relays.contains_key(relay) {
                    continue;
                }

                // Skip if relay is already assigned this pubkey
                if let Some(maybe_assignment) = self.relay_assignments.get(relay) {
                    if let Some(assignment) = maybe_assignment.value() {
                        if assignment.pubkeys.contains(pubkeyhex) {
                            continue;
                        }
                    }
                }

                // Add the score
                if let Some(mut entry) = scoreboard.get_mut(relay) {
                    *entry += score;
                }
            }
        }

        // Adjust all scores based on relay rank and relay success rate
        for mut score_entry in scoreboard.iter_mut() {
            let url = score_entry.key().to_owned();
            let score = score_entry.value_mut();
            if let Some(relay) = self.all_relays.get(&url) {
                let success_rate = relay.success_rate();
                let rank = (relay.rank as f32 * (1.3 * success_rate)) as u64;
                *score *= rank;
            }
        }

        let winner = scoreboard
            .iter()
            .max_by(|x, y| x.value().cmp(y.value()))
            .unwrap();
        let winning_url: RelayUrl = winner.key().to_owned();
        let winning_score: u64 = *winner.value();

        if winning_score == 0 {
            return Err(RelayPickerFailure::NoProgress);
        }

        // Get all the pubkeys this relay covers
        // (this includes ones we don't need)
        let over_covered_public_keys: Vec<PublicKeyHex> = self
            .person_relay_scores
            .iter()
            .filter(|elem| elem.value().iter().any(|ie| ie.0 == winning_url))
            .map(|elem| elem.key().to_owned())
            .collect();

        // Now only count the ones we need
        // and Decrement entries where we the winner covers them
        let mut covered_public_keys: Vec<PublicKeyHex> = Vec::new();
        for mut elem in self.pubkey_counts.iter_mut() {
            let pubkey = elem.key().to_owned();
            let count = elem.value_mut();
            if over_covered_public_keys.contains(&pubkey) && *count > 0 {
                covered_public_keys.push(pubkey.to_owned());
                *count -= 1;
            }
        }

        if covered_public_keys.is_empty() {
            return Err(RelayPickerFailure::NoProgress);
        }

        // Only keep pubkey_counts that are still > 0
        self.pubkey_counts.retain(|_, count| *count > 0);

        let assignment = RelayAssignment2 {
            relay_url: winning_url.clone(),
            pubkeys: covered_public_keys,
        };

        // Put assignment into relay_assignments
        if let Some(mut maybe_elem) =  self.relay_assignments.get_mut(&winning_url) {
            if maybe_elem.value().is_none() {
                *maybe_elem.value_mut() = Some(assignment);
            } else {
                // FIXME this could cause a panic, but it would mean we have bad code.
                maybe_elem.value_mut().as_mut().unwrap().merge_in(assignment).unwrap();
            }
        } else {
            self.relay_assignments.insert(winning_url.clone(), Some(assignment));
        }

        Ok(winning_url)
    }
}
