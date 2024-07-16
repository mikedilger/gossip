use crate::error::Error;
use crate::globals::GLOBALS;
use crate::relay;
use async_trait::async_trait;
use gossip_relay_picker::RelayPickerHooks;
use nostr_types::{PublicKey, RelayUrl, RelayUsage};

/// Hooks for the relay picker
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

    /// Returns all relays that this public key uses in the given RelayUsage
    async fn get_relays_for_pubkey(
        &self,
        pubkey: PublicKey,
        usage: RelayUsage,
    ) -> Result<Vec<(RelayUrl, u64)>, Error> {
        let write = usage == RelayUsage::Outbox;
        relay::get_best_relays_with_score(pubkey, write, 0)
    }

    /// Is the relay currently connected?
    fn is_relay_connected(&self, relay: &RelayUrl) -> bool {
        GLOBALS.connected_relays.contains_key(relay)
    }

    /// Returns the maximum number of relays that should be connected to at one time
    fn get_max_relays(&self) -> usize {
        GLOBALS.storage.read_setting_max_relays() as usize
    }

    /// Returns the number of relays each followed person's events should be pulled from
    /// Many people use 2 or 3 for redundancy.
    fn get_num_relays_per_person(&self) -> usize {
        GLOBALS.storage.read_setting_num_relays_per_person() as usize
    }

    /// Returns the public keys of all the people followed
    // this API name has become difficult..
    fn get_followed_pubkeys(&self) -> Vec<PublicKey> {
        // ..We actually want all the people subscribed, which is a bigger list
        GLOBALS.people.get_subscribed_pubkeys()
    }

    /// Adjusts the score for a given relay, perhaps based on relay-specific metrics
    fn adjust_score(&self, url: RelayUrl, score: u64) -> u64 {
        match GLOBALS.storage.read_relay(&url, None) {
            Err(_) => 0,
            Ok(Some(relay)) => {
                if relay.should_avoid() {
                    return 0;
                }
                let success_rate = relay.success_rate();
                let rank = (relay.rank as f32 * (1.3 * success_rate)) as u64;
                score * rank
            }
            Ok(None) => score,
        }
    }
}
