mod person1;
pub(crate) use person1::Person1;

mod person2;
pub use person2::Person2;

mod person3;
pub use person3::Person3;

mod person_list1;
pub use person_list1::PersonList1;

mod person_list_metadata1;
pub use person_list_metadata1::PersonListMetadata1;

mod person_list_metadata2;
pub use person_list_metadata2::PersonListMetadata2;

mod person_list_metadata3;
pub use person_list_metadata3::PersonListMetadata3;

mod person_relay1;
pub use person_relay1::PersonRelay1;

mod person_relay2;
pub use person_relay2::PersonRelay2;

mod relationship1;
pub use relationship1::Relationship1;

mod relationship_by_addr1;
pub use relationship_by_addr1::RelationshipByAddr1;

mod relationship_by_addr2;
pub use relationship_by_addr2::RelationshipByAddr2;

mod relationship_by_id1;
pub use relationship_by_id1::RelationshipById1;

mod relationship_by_id2;
pub use relationship_by_id2::RelationshipById2;

mod relay1;
pub use relay1::Relay1;

mod relay2;
pub use relay2::Relay2;

mod settings1;
pub(crate) use settings1::Settings1;

mod settings2;
pub(crate) use settings2::Settings2;

mod theme1;
pub(crate) use theme1::{Theme1, ThemeVariant1};

use crate::error::Error;
use nostr_types::{Id, PublicKey};

pub trait ByteRep: Sized {
    fn to_bytes(&self) -> Result<Vec<u8>, Error>;
    fn from_bytes(bytes: &[u8]) -> Result<Self, Error>;
}

impl ByteRep for Id {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Id(bytes.try_into()?))
    }
}

impl ByteRep for PublicKey {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.to_bytes())
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Self::from_bytes(bytes, false)?)
    }
}

pub trait Record: ByteRep {
    type Key: Copy + ByteRep;

    /// Create a new record
    fn new(k: Self::Key) -> Self;

    /// Get the key of a record
    fn key(&self) -> Self::Key;

    /// Stabilize a record prior to writing.
    /// Usually nothing needs to be done.
    fn stabilize(&mut self) { }
}
