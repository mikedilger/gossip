use crate::error::Error;
use crate::globals::GLOBALS;
use gossip_relay_picker::Direction;
use nostr_types::{Id, RelayInformationDocument, RelayUrl, Unixtime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relay {
    pub url: RelayUrl,
    pub success_count: u64,
    pub failure_count: u64,
    pub last_connected_at: Option<u64>,
    pub last_general_eose_at: Option<u64>,
    pub rank: u64,
    pub hidden: bool,
    pub usage_bits: u64,
    pub nip11: Option<RelayInformationDocument>,
    pub last_attempt_nip11: Option<u64>,
}

impl Relay {
    pub const READ: u64 = 1 << 0; // 1
    pub const WRITE: u64 = 1 << 1; // 2
    pub const ADVERTISE: u64 = 1 << 2; // 4
    pub const INBOX: u64 = 1 << 3; // 8            this is 'read' of kind 10002
    pub const OUTBOX: u64 = 1 << 4; // 16          this is 'write' of kind 10002
    pub const DISCOVER: u64 = 1 << 5; // 32

    pub fn new(url: RelayUrl) -> Relay {
        Relay {
            url,
            success_count: 0,
            failure_count: 0,
            last_connected_at: None,
            last_general_eose_at: None,
            rank: 3,
            hidden: false,
            usage_bits: 0,
            nip11: None,
            last_attempt_nip11: None,
        }
    }

    #[inline]
    pub fn set_usage_bits(&mut self, bits: u64) {
        self.usage_bits |= bits;
    }

    #[inline]
    pub fn clear_usage_bits(&mut self, bits: u64) {
        self.usage_bits &= !bits;
    }

    #[inline]
    pub fn adjust_usage_bit(&mut self, bit: u64, value: bool) {
        if value {
            self.set_usage_bits(bit);
        } else {
            self.clear_usage_bits(bit);
        }
    }

    #[inline]
    pub fn has_usage_bits(&self, bits: u64) -> bool {
        self.usage_bits & bits == bits
    }

    #[inline]
    pub fn attempts(&self) -> u64 {
        self.success_count + self.failure_count
    }

    #[inline]
    pub fn success_rate(&self) -> f32 {
        let attempts = self.attempts();
        if attempts == 0 {
            return 0.5;
        } // unknown, so we put it in the middle
        self.success_count as f32 / attempts as f32
    }

    /// This generates a "recommended_relay_url" for an 'e' tag.
    pub async fn recommended_relay_for_reply(reply_to: Id) -> Result<Option<RelayUrl>, Error> {
        let seen_on_relays: Vec<(RelayUrl, Unixtime)> =
            GLOBALS.storage.get_event_seen_on_relay(reply_to)?;

        let maybepubkey = GLOBALS.storage.read_setting_public_key();
        if let Some(pubkey) = maybepubkey {
            let my_inbox_relays: Vec<(RelayUrl, u64)> =
                GLOBALS.storage.get_best_relays(pubkey, Direction::Read)?;

            // Find the first-best intersection
            for mir in &my_inbox_relays {
                for sor in &seen_on_relays {
                    if mir.0 == sor.0 {
                        return Ok(Some(mir.0.clone()));
                    }
                }
            }

            // Else use my first inbox
            if let Some(mir) = my_inbox_relays.first() {
                return Ok(Some(mir.0.clone()));
            }

            // Else fall through to seen on relays only
        }

        if let Some(sor) = seen_on_relays.first() {
            return Ok(Some(sor.0.clone()));
        }

        Ok(None)
    }
}

/**
 *  Static helper functions
 */
impl Relay {
    pub fn domain_from_relay_url<'a>(relay: &'a RelayUrl) -> &'a str /* domain */ {
        let domain = relay.0.trim_start_matches("wss://");
        let domain = domain.trim_start_matches("ws://");
        let domain = domain.trim_end_matches('/');
        domain
    }
}
