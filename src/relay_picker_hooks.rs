use crate::globals::GLOBALS;
use crate::db::DbPersonRelay;
use crate::error::Error;
use async_trait::async_trait;
use gossip_relay_picker::{Direction, RelayPickerHooks};
use nostr_types::{PublicKeyHex, RelayUrl};

#[derive(Default)]
pub struct Hooks { }

#[async_trait]
impl RelayPickerHooks for Hooks {
    type Error = Error;

    /// Returns all relays available to be connected to
    fn get_all_relays(&self) -> Vec<RelayUrl> {
        GLOBALS.all_relays.iter().map(|elem| elem.key().to_owned()).collect()
    }

    /// Returns all relays that this public key uses in the given Direction
    async fn get_relays_for_pubkey(
        &self,
        pubkey: PublicKeyHex,
        direction: Direction,
    ) -> Result<Vec<(RelayUrl, u64)>, Error> {
        DbPersonRelay::get_best_relays(pubkey, direction).await
    }

    /// Is the relay currently connected?
    fn is_relay_connected(&self, relay: &RelayUrl) -> bool {
        GLOBALS.connected_relays.contains(relay)
    }

    /// Returns the maximum number of relays that should be connected to at one time
    fn get_max_relays(&self) -> usize {
        GLOBALS.settings.blocking_read().max_relays as usize
    }

    /// Returns the number of relays each followed person's events should be pulled from
    /// Many people use 2 or 3 for redundancy.
    fn get_num_relays_per_person(&self) -> usize {
        GLOBALS.settings.blocking_read().num_relays_per_person as usize
    }

    /// Returns the public keys of all the people followed
    fn get_followed_pubkeys(&self) -> Vec<PublicKeyHex> {
        GLOBALS.people.get_followed_pubkeys()
    }

    /// Adjusts the score for a given relay, perhaps based on relay-specific metrics
    fn adjust_score(&self, relay: RelayUrl, score: u64) -> u64 {
        if let Some(relay) = GLOBALS.all_relays.get(&relay) {
            let success_rate = relay.success_rate();
            let rank = (relay.rank as f32 * (1.3 * success_rate)) as u64;
            score * rank
        } else {
            score
        }
    }
}

