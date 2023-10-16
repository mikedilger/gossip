use speedy::{Readable, Writable};

/// Lists people can be added to
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Readable, Writable)]
#[repr(u8)]
pub enum PersonList1 {
    Muted = 0,
    Followed = 1,
    Priority = 2,

    // custom starts at 10 to leave space
    Custom(u8),
}

impl From<u8> for PersonList1 {
    fn from(u: u8) -> Self {
        match u {
            0 => PersonList1::Muted,
            1 => PersonList1::Followed,
            2 => PersonList1::Priority,
            u => PersonList1::Custom(u),
        }
    }
}

impl From<PersonList1> for u8 {
    fn from(e: PersonList1) -> u8 {
        match e {
            PersonList1::Muted => 0,
            PersonList1::Followed => 1,
            PersonList1::Priority => 2,
            PersonList1::Custom(u) => u,
        }
    }
}

impl PersonList1 {
    pub fn name(&self) -> String {
        match *self {
            PersonList1::Muted => "Muted".to_string(),
            PersonList1::Followed => "Followed".to_string(),
            PersonList1::Priority => "Priority".to_string(),
            PersonList1::Custom(u) => format!("Custom {}", u - 9),
        }
    }
}
