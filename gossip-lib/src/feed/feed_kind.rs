use crate::dm_channel::DmChannel;
use crate::globals::GLOBALS;
use crate::people::PersonList;
use nostr_types::{Id, PublicKey};

/// Kinds of feeds, with configuration parameteers
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeedKind {
    List(PersonList, bool), // with replies
    Bookmarks,
    Inbox(bool), // indirect
    Thread {
        id: Id, // FIXME, should be an EventReference
        referenced_by: Id,
        author: Option<PublicKey>,
    },
    Person(PublicKey),
    DmChat(DmChannel),
    Global,
}

impl std::fmt::Display for FeedKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeedKind::List(pl, _) => match GLOBALS.db().get_person_list_metadata(*pl) {
                Ok(Some(md)) => write!(f, "{}", md.title),
                _ => write!(f, "UNKNOWN"),
            },
            FeedKind::Bookmarks => write!(f, "Bookmarks"),
            FeedKind::Inbox(_) => write!(f, "Inbox"),
            FeedKind::Thread {
                id,
                referenced_by: _,
                author: _,
            } => write!(f, "Thread {}", crate::names::hex_id_short(&(*id).into())),
            FeedKind::Person(pk) => write!(f, "{}", crate::names::best_name_from_pubkey_lookup(pk)),
            FeedKind::DmChat(channel) => write!(f, "{}", channel.name()),
            FeedKind::Global => write!(f, "Global"),
        }
    }
}

impl FeedKind {
    // this is used to keep a set of feed anchors separate for each kind of feed.
    pub fn anchor_key(&self) -> String {
        match self {
            Self::List(personlist, _) => format!("list{}", personlist.as_u8()),
            Self::Bookmarks => "bookmarks".to_owned(),
            Self::Inbox(_) => "inbox".to_owned(),
            Self::Thread { .. } => "thread".to_owned(),
            Self::Person(pubkey) => format!("person{}", pubkey.as_hex_string()),
            Self::DmChat(_) => "dmchat".to_owned(),
            Self::Global => "global".to_owned(),
        }
    }

    pub fn can_load_more(&self) -> bool {
        match self {
            Self::List(_, _) => true,
            Self::Bookmarks => false, // always full
            Self::Inbox(_) => true,
            Self::Thread { .. } => false, // always full
            Self::Person(_) => true,
            Self::DmChat(_) => false, // always full
            Self::Global => true,
        }
    }
}
