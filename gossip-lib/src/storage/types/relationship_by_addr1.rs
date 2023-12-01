use nostr_types::PublicKey;
use speedy::{Readable, Writable};

/// A relationship between events by Address and Id
#[derive(Clone, Debug, PartialEq, Eq, Readable, Writable)]
pub enum RelationshipByAddr1 {
    // NIP-01, NIP-10 replies
    Reply,

    // NIP-09 Event Deletion
    Deletion { by: PublicKey, reason: String },

    // NIP-51 Lists
    ListBookmarks,

    // NIP-51 Lists
    Curation,

    // communities
    // interests
    // emojis

    // NIP-53
    LiveChatMessage,

    // NIP-58
    BadgeAward,

    // NIP-72 Moderated Communities (Reddit-style)
    // PostedToCommunity,

    // NIP-89 Recommended Application Handlers
    HandlerRecommendation,
}
