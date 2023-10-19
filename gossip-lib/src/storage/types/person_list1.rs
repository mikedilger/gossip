use crate::globals::GLOBALS;
use speedy::{Readable, Writable};

/// Lists people can be added to
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Readable, Writable)]
#[repr(u8)]
pub enum PersonList1 {
    Muted = 0,
    Followed = 1,

    // custom starts at 10 to leave space
    Custom(u8),
}

impl From<u8> for PersonList1 {
    fn from(u: u8) -> Self {
        match u {
            0 => PersonList1::Muted,
            1 => PersonList1::Followed,
            u => PersonList1::Custom(u),
        }
    }
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
    /// Custom lists (from 0-9, humans number them from 1-10)
    pub fn custom(index: u8) -> Option<PersonList1> {
        if index > 9 {
            None
        } else {
            Some(PersonList1::Custom(index + 10))
        }
    }

    pub fn name(&self) -> String {
        match *self {
            PersonList1::Muted => "Muted".to_string(),
            PersonList1::Followed => "Followed".to_string(),
            PersonList1::Custom(u) => {
                if (10..=19).contains(&u) {
                    GLOBALS.storage.read_setting_custom_person_list_names()[u as usize - 10].clone()
                } else if u > 19 {
                    format!("Custom List {}", u - 9) // humans count from 1
                } else {
                    format!("Undefined list in slot={}", u)
                }
            }
        }
    }

    pub fn subscribe(&self) -> bool {
        match *self {
            PersonList1::Muted => false,
            PersonList1::Followed => true,
            PersonList1::Custom(_) => true,
        }
    }
}
