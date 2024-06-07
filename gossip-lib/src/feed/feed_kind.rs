use crate::dm_channel::DmChannel;
use crate::globals::GLOBALS;
use crate::people::PersonList;
use nostr_types::{Id, PublicKey};

/// Kinds of feeds, with configuration parameteers
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeedKind {
    List(PersonList, bool), // with replies
    Inbox(bool),            // indirect
    Thread {
        id: Id, // FIXME, should be an EventReference
        referenced_by: Id,
        author: Option<PublicKey>,
    },
    Person(PublicKey),
    DmChat(DmChannel),
}

impl std::fmt::Display for FeedKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeedKind::DmChat(channel) => write!(f, "{}", channel.name()),
            FeedKind::List(pl, _) => match GLOBALS.storage.get_person_list_metadata(*pl) {
                Ok(Some(md)) => write!(f, "{}", md.title),
                _ => write!(f, "UNKNOWN"),
            },
            FeedKind::Inbox(_) => write!(f, "Inbox"),
            FeedKind::Thread {
                id,
                referenced_by: _,
                author: _,
            } => write!(f, "Thread {}", crate::names::hex_id_short(&(*id).into())),
            FeedKind::Person(pk) => write!(f, "{}", crate::names::best_name_from_pubkey_lookup(pk)),
        }
    }
}

impl FeedKind {
    pub fn simple_string(&self) -> &'static str {
        match self {
            Self::List(_, _) => "list",
            Self::Inbox(_) => "inbox",
            Self::Thread { .. } => "thread",
            Self::Person(_) => "person",
            Self::DmChat(_) => "dmchat",
        }
    }

    pub fn can_load_more(&self) -> bool {
        match self {
            Self::List(_, _) => true,
            Self::Inbox(_) => true,
            Self::Thread { .. } => false, // always full
            Self::Person(_) => true,
            Self::DmChat(_) => false, // always full
        }
    }
}
