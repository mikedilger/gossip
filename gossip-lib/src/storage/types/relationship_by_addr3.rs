use super::RelationshipByAddr2;
use nostr_types::{MilliSatoshi, PublicKey};
use speedy::{Readable, Writable};

/// A relationship between events by Address and Id
#[derive(Clone, Debug, PartialEq, Eq, Readable, Writable)]
pub enum RelationshipByAddr3 {
    // NIP-01, NIP-10 replies
    RepliesTo,

    // Annotation
    Annotates,

    // NIP-09 Event Deletion
    Deletes { by: PublicKey, reason: String },

    // NIP-32 Labeling
    Labels { label: String, namespace: String },

    // NIP-51 Lists
    Bookmarks,

    // NIP-51 Lists
    Curates,

    // communities
    // interests
    // emojis

    // NIP-53
    ChatsWithin,

    // NIP-58
    AwardsBadge,

    // NIP-72 Moderated Communities (Reddit-style)
    // CommunityPostsWithin,

    // NIP-89 Recommended Application Handlers
    RecommendsHandler,

    // NIP-57 Zap
    Zaps { by: PublicKey, amount: MilliSatoshi },
}

impl From<RelationshipByAddr2> for RelationshipByAddr3 {
    fn from(two: RelationshipByAddr2) -> RelationshipByAddr3 {
        match two {
            RelationshipByAddr2::RepliesTo => RelationshipByAddr3::RepliesTo,
            RelationshipByAddr2::Annotates => RelationshipByAddr3::Annotates,
            RelationshipByAddr2::Deletes { by, reason } => {
                RelationshipByAddr3::Deletes { by, reason }
            }
            RelationshipByAddr2::Labels { label, namespace } => {
                RelationshipByAddr3::Labels { label, namespace }
            }
            RelationshipByAddr2::Bookmarks => RelationshipByAddr3::Bookmarks,
            RelationshipByAddr2::Curates => RelationshipByAddr3::Curates,
            RelationshipByAddr2::ChatsWithin => RelationshipByAddr3::ChatsWithin,
            RelationshipByAddr2::AwardsBadge => RelationshipByAddr3::AwardsBadge,
            RelationshipByAddr2::RecommendsHandler => RelationshipByAddr3::RecommendsHandler,
        }
    }
}
