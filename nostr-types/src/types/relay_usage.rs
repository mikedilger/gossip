use std::convert::TryFrom;

/// A way that a user uses a Relay
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[repr(u32)]
pub enum RelayUsage {
    /// User seeks events here if they are not otherwise found
    FallbackRead = 1 << 0,

    /// User writes here but does not advertise it
    Archive = 1 << 1,

    // was a relay usage flag in gossip, but was retired
    // Advertise = 1 << 2,
    /// User accepts posts here from the public that tag them
    Inbox = 1 << 3,

    /// User posts here for the public
    Outbox = 1 << 4,

    /// User seeks relay lists here (index, discover)
    Directory = 1 << 5,

    // is used as SPAMSAFE bit in gossip so reserved, but isn't a relay usage
    // ReservedSpamsafe = 1 << 6,
    /// User accepts DMs here
    Dm = 1 << 7,

    /// user stores and reads back their own configurations here
    Config = 1 << 8,

    /// User does NIP-50 SEARCH here
    Search = 1 << 9,
}

impl TryFrom<u32> for RelayUsage {
    type Error = ();

    fn try_from(u: u32) -> Result<RelayUsage, ()> {
        match u {
            1 => Ok(RelayUsage::FallbackRead),
            2 => Ok(RelayUsage::Archive),
            8 => Ok(RelayUsage::Inbox),
            16 => Ok(RelayUsage::Outbox),
            32 => Ok(RelayUsage::Directory),
            128 => Ok(RelayUsage::Dm),
            256 => Ok(RelayUsage::Config),
            512 => Ok(RelayUsage::Search),
            _ => Err(()),
        }
    }
}

/// The ways that a user uses a Relay
///
// See also https://github.com/mikedilger/gossip/blob/master/gossip-lib/src/storage/types/relay3.rs
// See also https://github.com/nostr-protocol/nips/issues/1282 for possible future entries
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct RelayUsageSet(u32);

impl RelayUsageSet {
    const MASK: u32 = RelayUsage::FallbackRead as u32
        | RelayUsage::Archive as u32
        | RelayUsage::Inbox as u32
        | RelayUsage::Outbox as u32
        | RelayUsage::Directory as u32
        | RelayUsage::Dm as u32
        | RelayUsage::Config as u32
        | RelayUsage::Search as u32;

    /// Create a new empty RelayUsageSet
    pub const fn new_empty() -> Self {
        RelayUsageSet(0)
    }

    /// Create a new RelayUsageSet with all usages
    pub const fn new_all() -> Self {
        Self(Self::MASK)
    }

    /// Get the u32 bitflag representation
    pub const fn bits(&self) -> u32 {
        self.0
    }

    /// Set from a u32 bitflag representation. If any unknown bits are set,
    /// this will return None
    pub const fn from_bits(bits: u32) -> Option<RelayUsageSet> {
        if bits & !Self::MASK != 0 {
            None
        } else {
            Some(RelayUsageSet(bits))
        }
    }

    /// Set from a u32 bitflag representation. If any unknown bits are set,
    /// they will be cleared
    pub const fn from_bits_truncate(bits: u32) -> RelayUsageSet {
        RelayUsageSet(bits & Self::MASK)
    }

    /// Whether all bits are unset
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Whether all defined bits are set
    pub const fn is_all(&self) -> bool {
        self.0 & Self::MASK == Self::MASK
    }

    /// Whether any usage in other is also in Self
    pub const fn intersects(&self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Whether all usages in other are in Self
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Has a RelayUsage set
    pub fn has_usage(&mut self, ru: RelayUsage) -> bool {
        self.0 & ru as u32 == ru as u32
    }

    /// Add a RelayUsage to Self
    pub fn add_usage(&mut self, ru: RelayUsage) {
        self.0 |= ru as u32
    }

    /// Remove a RelayUsage to Self
    pub fn remove_usage(&mut self, ru: RelayUsage) {
        self.0 &= !(ru as u32)
    }
}
