use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Id, RelayInformationDocument, RelayUrl, Unixtime};
use serde::{Deserialize, Serialize};

// THIS IS HISTORICAL FOR MIGRATIONS AND THE STRUCTURES SHOULD NOT BE EDITED

/// A relay record
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Relay2 {
    /// The url
    pub url: RelayUrl,

    /// How many times we successfully connected
    pub success_count: u64,

    /// How many times we failed to connect, plus we also count when
    /// the relay drops us without us requesting that
    pub failure_count: u64,

    /// When we last connected to the relay
    pub last_connected_at: Option<u64>,

    /// When the relay last gave us an EOSE on the general feed
    pub last_general_eose_at: Option<u64>,

    /// What rank the user applied to this relay.
    /// Valid ranks go from 0 to 9, with a default of 3. 0 means do not use.
    pub rank: u64,

    /// If this should be hidden in the UI
    pub hidden: bool,

    /// What usage this relay provides to the user
    /// (hidden because 'advertise' may be set which would interfere with simple
    /// .cmp and zero tests)
    pub(in crate::storage) usage_bits: u64,

    /// The NIP-11 for this relay
    pub nip11: Option<RelayInformationDocument>,

    /// The last time we attempted to fetch the NIP-11 for this relay
    /// (in unixtime seconds)
    pub last_attempt_nip11: Option<u64>,

    /// If the user allows connection to this relay
    /// None: Ask (Default)
    /// Some(false): Never
    /// Some(true): Always
    pub allow_connect: Option<bool>,

    /// If the user allows this relay to AUTH them
    /// None: Ask (Default)
    /// Some(false): Never
    /// Some(true): Always
    pub allow_auth: Option<bool>,
}

impl Relay2 {
    pub const READ: u64 = 1 << 0; // 1
    pub const WRITE: u64 = 1 << 1; // 2
    const ADVERTISE: u64 = 1 << 2; // 4 // RETIRED
    pub const INBOX: u64 = 1 << 3; // 8            this is 'read' of kind 10002
    pub const OUTBOX: u64 = 1 << 4; // 16          this is 'write' of kind 10002
    pub const DISCOVER: u64 = 1 << 5; // 32
    pub const SPAMSAFE: u64 = 1 << 6; // 64
    pub const DM: u64 = 1 << 7; // 128             this is of kind 10050

    pub fn new(url: RelayUrl) -> Self {
        Self {
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
            allow_connect: None,
            allow_auth: None,
        }
    }

    #[inline]
    pub fn get_usage_bits(&self) -> u64 {
        // Automatically clear any residual ADVERTISE bit
        // ( so that simple cmp() and =0 still work... but you should use
        //   the new has_any_usage_bit() instead to be safe )
        self.usage_bits & !Self::ADVERTISE
    }

    #[inline]
    pub fn get_usage_bits_for_sorting(&self) -> u64 {
        let mut output: u64 = 0;
        if self.has_usage_bits(Self::READ) {
            output |= 1 << 6;
        }
        if self.has_usage_bits(Self::WRITE) {
            output |= 1 << 5;
        }
        if self.has_usage_bits(Self::INBOX) {
            output |= 1 << 4;
        }
        if self.has_usage_bits(Self::OUTBOX) {
            output |= 1 << 3;
        }
        if self.has_usage_bits(Self::DM) {
            output |= 1 << 2;
        }
        // DISCOVER and SPAMSAFE shouldn't affect sort
        output
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
    pub fn has_any_usage_bit(&self) -> bool {
        let all = Self::READ | Self::WRITE | Self::INBOX | Self::OUTBOX | Self::DISCOVER | Self::DM;
        self.usage_bits & all != 0
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

    pub fn is_good_for_advertise(&self) -> bool {
        self.has_usage_bits(Self::INBOX)
            || self.has_usage_bits(Self::OUTBOX)
            || self.has_usage_bits(Self::DISCOVER)
            || (self.rank > 0 && self.success_rate() > 0.50 && self.success_count > 15)
    }

    /// This generates a "recommended_relay_url" for an 'e' tag.
    pub fn recommended_relay_for_reply(reply_to: Id) -> Result<Option<RelayUrl>, Error> {
        let seen_on_relays: Vec<(RelayUrl, Unixtime)> =
            GLOBALS.storage.get_event_seen_on_relay(reply_to)?;

        let maybepubkey = GLOBALS.storage.read_setting_public_key();
        if let Some(pubkey) = maybepubkey {
            let my_inbox_relays: Vec<RelayUrl> =
                GLOBALS.storage.get_best_relays(pubkey, false, 0)?;

            // Find the first-best intersection
            for mir in &my_inbox_relays {
                for sor in &seen_on_relays {
                    if *mir == sor.0 {
                        return Ok(Some(mir.clone()));
                    }
                }
            }

            // Else use my first inbox
            if let Some(mir) = my_inbox_relays.first() {
                return Ok(Some(mir.clone()));
            }

            // Else fall through to seen on relays only
        }

        if let Some(sor) = seen_on_relays.first() {
            return Ok(Some(sor.0.clone()));
        }

        Ok(None)
    }
}
