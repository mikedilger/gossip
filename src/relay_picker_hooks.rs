use crate::error::Error;
use crate::globals::GLOBALS;
use async_trait::async_trait;
use gossip_relay_picker::{Direction, RelayPickerHooks};
use nostr_types::{PublicKey, RelayUrl};

#[derive(Default)]
pub struct Hooks {}

#[async_trait]
impl RelayPickerHooks for Hooks {
    type Error = Error;

    /// Returns all relays available to be connected to
    fn get_all_relays(&self) -> Vec<RelayUrl> {
        match GLOBALS.storage.filter_relays(|_| true) {
            Err(_) => vec![],
            Ok(vec) => vec.iter().map(|elem| elem.url.to_owned()).collect(),
        }
    }

    /// Returns all relays that this public key uses in the given Direction
    async fn get_relays_for_pubkey(
        &self,
        pubkey: PublicKey,
        direction: Direction,
    ) -> Result<Vec<(RelayUrl, u64)>, Error> {
        GLOBALS.storage.get_best_relays(pubkey, direction)
    }

    /// Is the relay currently connected?
    fn is_relay_connected(&self, relay: &RelayUrl) -> bool {
        GLOBALS.connected_relays.contains_key(relay)
    }

    /// Returns the maximum number of relays that should be connected to at one time
    fn get_max_relays(&self) -> usize {
        GLOBALS.settings.read().max_relays as usize
    }

    /// Returns the number of relays each followed person's events should be pulled from
    /// Many people use 2 or 3 for redundancy.
    fn get_num_relays_per_person(&self) -> usize {
        GLOBALS.settings.read().num_relays_per_person as usize
    }

    /// Returns the public keys of all the people followed
    fn get_followed_pubkeys(&self) -> Vec<PublicKey> {
        GLOBALS.people.get_followed_pubkeys()
    }

    /// Adjusts the score for a given relay, perhaps based on relay-specific metrics
    fn adjust_score(&self, url: RelayUrl, score: u64) -> u64 {
        match GLOBALS.storage.read_relay(&url) {
            Err(_) => 0,
            Ok(Some(relay)) => {
                let success_rate = relay.success_rate();
                let rank = (relay.rank as f32 * (1.3 * success_rate)) as u64;
                score * rank
            }
            Ok(None) => score,
        }
    }
}
