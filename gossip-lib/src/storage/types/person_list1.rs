use crate::globals::GLOBALS;
use nostr_types::EventKind;
use speedy::{Readable, Writable};

/// Lists people can be added to
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Readable, Writable)]
#[repr(u8)]
pub enum PersonList1 {
    Muted = 0,
    Followed = 1,
    // Custom starts at 2.  If we later need more well-known values, we will
    // run them from 255 and work down.
    Custom(u8),
}

impl From<PersonList1> for u8 {
    fn from(e: PersonList1) -> u8 {
        match e {
            PersonList1::Muted => 0,
            PersonList1::Followed => 1,
            PersonList1::Custom(u) => u,
        }
    }
}

impl PersonList1 {
    pub(in crate::storage) fn from_u8(u: u8) -> Self {
        match u {
            0 => PersonList1::Muted,
            1 => PersonList1::Followed,
            u => PersonList1::Custom(u),
        }
    }

    pub fn as_u8(&self) -> u8 {
        (*self).into()
    }

    pub fn from_number(number: u8) -> Option<Self> {
        let list = Self::from_u8(number);
        if matches!(list, PersonList1::Custom(_)) {
            match GLOBALS.db().get_person_list_metadata(list) {
                Ok(Some(_)) => Some(list),
                _ => None,
            }
        } else {
            Some(list)
        }
    }

    /// Get the event kind matching this PersonList1
    pub fn event_kind(&self) -> EventKind {
        match *self {
            PersonList1::Followed => EventKind::ContactList,
            PersonList1::Muted => EventKind::MuteList,
            PersonList1::Custom(_) => EventKind::FollowSets,
        }
    }

    /// Should we subscribe to events from people in this list?
    pub fn subscribe(&self) -> bool {
        !matches!(*self, PersonList1::Muted)
    }
}
