use nostr_types::{MilliSatoshi, PublicKey};
use speedy::{Readable, Writable};

use super::RelationshipById1;

/// A relationship between events by Ids
#[derive(Clone, Debug, PartialEq, Eq, Readable, Writable)]
pub enum RelationshipById2 {
    // NIP-01, NIP-10 replies
    RepliesTo,

    // Annotation
    Annotates,

    // NIP-18 Reposts
    Reposts,

    // NIP-18 Quotes
    Quotes,

    // NIP-03 OpenTimestamps Attestations for Events
    Timestamps,

    // NIP-09 Event Deletion
    Deletes { by: PublicKey, reason: String },

    // NIP-25 Reactions
    ReactsTo { by: PublicKey, reaction: String },

    // NIP-32 Labeling
    Labels { label: String, namespace: String },

    // NIP-51 Lists
    Mutes,

    // NIP-51 Lists
    Pins,

    // NIP-51 Lists
    Bookmarks,

    // NIP-51 Lists
    Curates,

    // NIP-56 Reporting
    Reports(String),

    // NIP-57 Lightning Zaps
    Zaps { by: PublicKey, amount: MilliSatoshi },

    // NIP-72 Moderated Communities (Reddit-style)
    // Approves { in_community: EventAddr },

    // NIP-90 Data Vending Machines
    SuppliesJobResult,
}

impl From<RelationshipById1> for RelationshipById2 {
    fn from(one: RelationshipById1) -> RelationshipById2 {
        match one {
            RelationshipById1::Reply => RelationshipById2::RepliesTo,
            RelationshipById1::Timestamp => RelationshipById2::Timestamps,
            RelationshipById1::Deletion { by, reason } => RelationshipById2::Deletes { by, reason },
            RelationshipById1::Reaction { by, reaction } => {
                RelationshipById2::ReactsTo { by, reaction }
            }
            RelationshipById1::Labels { label, namespace } => {
                RelationshipById2::Labels { label, namespace }
            }
            RelationshipById1::ListMutesThread => RelationshipById2::Mutes,
            RelationshipById1::ListPins => RelationshipById2::Pins,
            RelationshipById1::ListBookmarks => RelationshipById2::Bookmarks,
            RelationshipById1::Curation => RelationshipById2::Curates,
            RelationshipById1::Reports(s) => RelationshipById2::Reports(s),
            RelationshipById1::ZapReceipt { by, amount } => RelationshipById2::Zaps { by, amount },
            RelationshipById1::JobResult => RelationshipById2::SuppliesJobResult,
        }
    }
}
