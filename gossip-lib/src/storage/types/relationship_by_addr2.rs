use nostr_types::PublicKey;
use speedy::{Readable, Writable};

use super::RelationshipByAddr1;

/// A relationship between events by Address and Id
#[derive(Clone, Debug, PartialEq, Eq, Readable, Writable)]
pub enum RelationshipByAddr2 {
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
}

impl From<RelationshipByAddr1> for RelationshipByAddr2 {
    fn from(one: RelationshipByAddr1) -> RelationshipByAddr2 {
        match one {
            RelationshipByAddr1::Reply => RelationshipByAddr2::RepliesTo,
            RelationshipByAddr1::Deletion { by, reason } => {
                RelationshipByAddr2::Deletes { by, reason }
            }
            RelationshipByAddr1::ListBookmarks => RelationshipByAddr2::Bookmarks,
            RelationshipByAddr1::Curation => RelationshipByAddr2::Curates,
            RelationshipByAddr1::LiveChatMessage => RelationshipByAddr2::ChatsWithin,
            RelationshipByAddr1::BadgeAward => RelationshipByAddr2::AwardsBadge,
            RelationshipByAddr1::HandlerRecommendation => RelationshipByAddr2::RecommendsHandler,
        }
    }
}
