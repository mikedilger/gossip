/// Lists people can be added to
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[repr(u8)]
pub enum PersonList1 {
    Muted = 0,
    Followed = 1,
    Priority = 2,
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
