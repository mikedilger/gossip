use nostr_types::{MilliSatoshi, PublicKey};
use speedy::{Readable, Writable};

/// A relationship between events by Ids
#[derive(Clone, Debug, PartialEq, Eq, Readable, Writable)]
pub enum RelationshipById1 {
    // NIP-01, NIP-10 replies
    Reply,

    // NIP-03 OpenTimestamps Attestations for Events
    Timestamp,

    // NIP-09 Event Deletion
    Deletion { by: PublicKey, reason: String },

    // NIP-25 Reactions
    Reaction { by: PublicKey, reaction: String },

    // NIP-32 Labeling
    Labels { label: String, namespace: String },

    // NIP-51 Lists
    ListMutesThread,

    // NIP-51 Lists
    ListPins,

    // NIP-51 Lists
    ListBookmarks,

    // NIP-51 Lists
    Curation,

    // NIP-56 Reporting
    Reports(String),

    // NIP-57 Lightning Zaps
    ZapReceipt { by: PublicKey, amount: MilliSatoshi },

    // NIP-72 Moderated Communities (Reddit-style)
    // Approves { in_community: NAddr },

    // NIP-90 Data Vending Machines
    JobResult,
}
