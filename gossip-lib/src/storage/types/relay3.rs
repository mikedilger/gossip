use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{RelayInformationDocument, RelayUrl, Unixtime};
use serde::{Deserialize, Serialize};

// THIS IS HISTORICAL FOR MIGRATIONS AND THE STRUCTURES SHOULD NOT BE EDITED

/// A relay record
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Relay3 {
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

    /// Avoid until this timestamp
    pub avoid_until: Option<Unixtime>,
}

impl Relay3 {
    pub const READ: u64 = 1 << 0; // 1
    pub const WRITE: u64 = 1 << 1; // 2
    const ADVERTISE: u64 = 1 << 2; // 4 // RETIRED
    pub const INBOX: u64 = 1 << 3; // 8            this is 'read' of kind 10002
    pub const OUTBOX: u64 = 1 << 4; // 16          this is 'write' of kind 10002
    pub const DISCOVER: u64 = 1 << 5; // 32
    pub const SPAMSAFE: u64 = 1 << 6; // 64
    pub const DM: u64 = 1 << 7; // 128             this is of kind 10050
    pub const GLOBAL: u64 = 1 << 8; // 256

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
            avoid_until: None,
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

    // This only includes main bits that people see in their flags
    // (excludes retired ADVERTISED, SPAMSAFE and GLOBAL)
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

    pub fn should_avoid(&self) -> bool {
        #[allow(clippy::if_same_then_else)]
        if self.rank == 0 {
            true
        } else if GLOBALS
            .db()
            .read_setting_relay_connection_requires_approval()
            && self.allow_connect == Some(false)
        {
            true
        } else if crate::storage::Storage::url_is_banned(&self.url) {
            true
        } else if let Some(when) = self.avoid_until {
            when >= Unixtime::now()
        } else {
            false
        }
    }

    pub fn is_good_for_advertise(&self) -> bool {
        if self.should_avoid() {
            return false;
        }

        self.has_usage_bits(Self::INBOX)
            || self.has_usage_bits(Self::OUTBOX)
            || self.has_usage_bits(Self::DISCOVER)
            || (self.rank > 0 && self.success_rate() > 0.50 && self.success_count > 15)
    }

    /// This gives a pure score for the relay outside of context
    ///
    /// Output ranges from 0.0 (worst) to 1.0 (best)
    ///
    /// Typical good relays still only score about 0.3, simply because rank goes so high.
    ///
    /// If `None` is returned, do not use this relay.
    pub fn score(&self) -> f32 {
        if self.should_avoid() {
            return 0.0;
        }

        let mut score: f32 = 1.0;

        // Adjust by rank:
        //   1 = 0.11111
        //   3 = 0.33333
        //   5 = 0.55555
        //   9 = 1.0
        score *= self.rank as f32 / 9.0;

        // Adjust by success rate (max penalty of cutting in half)
        score *= 0.5 + 0.5 * self.success_rate();

        // We don't penalize low-attempt relays even as they are less reliable
        // because we want to let new relays establish.

        score
    }

    /// This also checks if we are already connected to a relay and those scores
    /// are doubled (and normalized to 0.0 to 1.0)
    pub fn score_plus_connected(&self) -> f32 {
        let score = self.score();
        if GLOBALS.connected_relays.contains_key(&self.url) {
            score
        } else {
            score * 0.5
        }
    }

    pub fn choose_relays<F>(bits: u64, f: F) -> Result<Vec<Relay3>, Error>
    where
        F: Fn(&Relay3) -> bool,
    {
        GLOBALS
            .db()
            .filter_relays(|r| r.has_usage_bits(bits) && !r.should_avoid() && f(r))
    }

    pub fn choose_relay_urls<F>(bits: u64, f: F) -> Result<Vec<RelayUrl>, Error>
    where
        F: Fn(&Relay3) -> bool,
    {
        Ok(GLOBALS
            .db()
            .filter_relays(|r| r.has_usage_bits(bits) && !r.should_avoid() && f(r))?
            .iter()
            .map(|r| r.url.clone())
            .collect())
    }
}
