use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use heed::RwTxn;
use nostr_types::EventKind;
use speedy::{Readable, Writable};

/// Lists people can be added to
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Readable, Writable)]
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

    pub fn from_number(number: u8) -> Option<Self> {
        let map = GLOBALS.storage.read_setting_custom_person_list_map();
        for k in map.keys() {
            if *k == number {
                return Some(Self::from_u8(number));
            }
        }
        None
    }

    /// Translate a name (d-tag) to a PersonList1
    pub fn from_name(name: &str) -> Option<PersonList1> {
        let map = GLOBALS.storage.read_setting_custom_person_list_map();
        for (k, v) in map.iter() {
            if v == name {
                return Some(Self::from_u8(*k));
            }
        }
        None
    }

    /// All Allocated PersonList1s
    pub fn all_lists() -> Vec<(PersonList1, String)> {
        let mut output: Vec<(PersonList1, String)> = vec![];
        let map = GLOBALS.storage.read_setting_custom_person_list_map();
        for (k, v) in map.iter() {
            match k {
                0 => output.push((PersonList1::Muted, v.clone())),
                1 => output.push((PersonList1::Followed, v.clone())),
                _ => output.push((PersonList1::Custom(*k), v.clone())),
            }
        }
        output
    }

    /// Allocate a new PersonList1 with the given name
    pub fn allocate(name: &str, txn: Option<&mut RwTxn<'_>>) -> Result<PersonList1, Error> {
        // Do not allocate for well-known names
        if name == "Followed" {
            return Ok(PersonList1::Followed);
        } else if name == "Muted" {
            return Ok(PersonList1::Muted);
        }

        let mut map = GLOBALS.storage.read_setting_custom_person_list_map();

        // Check if it already exists to prevent duplicates
        for (k, v) in map.iter() {
            if v == name {
                return Ok(PersonList1::Custom(*k));
            }
        }

        // Find a slot and allocate
        for i in 2..255 {
            if map.contains_key(&i) {
                continue;
            }
            map.insert(i, name.to_owned());
            GLOBALS
                .storage
                .write_setting_custom_person_list_map(&map, txn)?;
            return Ok(PersonList1::Custom(i));
        }

        Err(ErrorKind::NoSlotsRemaining.into())
    }

    /// Deallocate this PersonList1
    pub fn deallocate(&self, txn: Option<&mut RwTxn<'_>>) -> Result<(), Error> {
        if !GLOBALS.storage.get_people_in_list(*self, None)?.is_empty() {
            Err(ErrorKind::ListIsNotEmpty.into())
        } else {
            if let PersonList1::Custom(i) = self {
                let mut map = GLOBALS.storage.read_setting_custom_person_list_map();
                map.remove(i);
                GLOBALS
                    .storage
                    .write_setting_custom_person_list_map(&map, txn)?;
                Ok(())
            } else {
                Err(ErrorKind::ListIsWellKnown.into())
            }
        }
    }

    pub fn rename(&self, name: &str, txn: Option<&mut RwTxn<'_>>) -> Result<(), Error> {
        if let PersonList1::Custom(i) = self {
            let mut map = GLOBALS.storage.read_setting_custom_person_list_map();
            map.insert(*i, name.to_owned());
            GLOBALS
                .storage
                .write_setting_custom_person_list_map(&map, txn)?;
            Ok(())
        } else {
            Err(ErrorKind::ListIsWellKnown.into())
        }
    }

    /// Get the name (d-tag) of this PersonList1
    pub fn name(&self) -> String {
        match *self {
            PersonList1::Muted => "Muted".to_string(),
            PersonList1::Followed => "Followed".to_string(),
            PersonList1::Custom(u) => {
                let map = GLOBALS.storage.read_setting_custom_person_list_map();
                match map.get(&u) {
                    Some(name) => name.to_owned(),
                    None => "Unallocated".to_owned(),
                }
            }
        }
    }

    /// Get the event kind matching this PersonList1
    pub fn event_kind(&self) -> EventKind {
        match *self {
            PersonList1::Followed => EventKind::ContactList,
            PersonList1::Muted => EventKind::MuteList,
            PersonList1::Custom(_) => EventKind::CategorizedPeopleList,
        }
    }

    /// Should we subscribe to events from people in this list?
    pub fn subscribe(&self) -> bool {
        !matches!(*self, PersonList1::Muted)
    }
}
