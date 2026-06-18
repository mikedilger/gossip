use crate::Error;
use derive_more::{AsMut, AsRef, Deref, Display, From, FromStr, Into};
use serde::de::{Deserializer, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
#[cfg(feature = "speedy")]
use speedy::{Readable, Writable};
use std::fmt;

/// An event identifier, constructed as a SHA256 hash of the event fields according to NIP-01
#[derive(
    AsMut, AsRef, Clone, Copy, Debug, Deref, Eq, From, Hash, Into, Ord, PartialEq, PartialOrd,
)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct Id(pub [u8; 32]);

impl Id {
    /// Render into a hexadecimal string
    ///
    /// Consider converting `.into()` an `IdHex` which is a wrapped type rather than a naked `String`
    pub fn as_hex_string(&self) -> String {
        hex::encode(self.0)
    }

    /// Create from a hexadecimal string
    pub fn try_from_hex_string(v: &str) -> Result<Id, Error> {
        let vec: Vec<u8> = hex::decode(v)?;
        Ok(Id(vec
            .try_into()
            .map_err(|_| Error::WrongLengthHexString)?))
    }

    /// Export as a bech32 encoded string ("note")
    pub fn as_bech32_string(&self) -> String {
        bech32::encode::<bech32::Bech32>(*crate::HRP_NOTE, &self.0).unwrap()
    }

    /// Import from a bech32 encoded string ("note")
    pub fn try_from_bech32_string(s: &str) -> Result<Id, Error> {
        let data = bech32::decode(s)?;
        if data.0 != *crate::HRP_NOTE {
            Err(Error::WrongBech32(
                crate::HRP_NOTE.to_lowercase(),
                data.0.to_lowercase(),
            ))
        } else if data.1.len() != 32 {
            Err(Error::InvalidId)
        } else {
            match <[u8; 32]>::try_from(data.1) {
                Ok(array) => Ok(Id(array)),
                _ => Err(Error::InvalidId),
            }
        }
    }

    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) fn mock() -> Id {
        Id::try_from_hex_string("5df64b33303d62afc799bdc36d178c07b2e1f0d824f31b7dc812219440affab6")
            .unwrap()
    }
}

impl Serialize for Id {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(self.0))
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(IdVisitor)
    }
}

struct IdVisitor;

impl Visitor<'_> for IdVisitor {
    type Value = Id;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a lowercase hexadecimal string representing 32 bytes")
    }

    fn visit_str<E>(self, v: &str) -> Result<Id, E>
    where
        E: serde::de::Error,
    {
        let vec: Vec<u8> = hex::decode(v).map_err(|e| serde::de::Error::custom(format!("{e}")))?;

        Ok(Id(vec.try_into().map_err(|e: Vec<u8>| {
            E::custom(format!(
                "Id is not 32 bytes long. Was {} bytes long",
                e.len()
            ))
        })?))
    }
}

/// An event identifier, constructed as a SHA256 hash of the event fields according to NIP-01, as a hex string
///
/// You can convert from an `Id` into this with `From`/`Into`.  You can convert this back to an `Id` with `TryFrom`/`TryInto`.
#[derive(
    AsMut,
    AsRef,
    Clone,
    Debug,
    Deref,
    Display,
    Eq,
    From,
    FromStr,
    Hash,
    Into,
    Ord,
    PartialEq,
    PartialOrd,
)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct IdHex(String);

impl IdHex {
    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) fn mock() -> IdHex {
        From::from(Id::mock())
    }

    /// Try from &str
    pub fn try_from_str(s: &str) -> Result<IdHex, Error> {
        Self::try_from_string(s.to_owned())
    }

    /// Try from String
    pub fn try_from_string(s: String) -> Result<IdHex, Error> {
        if s.len() != 64 {
            return Err(Error::InvalidId);
        }
        let vec: Vec<u8> = hex::decode(&s)?;
        if vec.len() != 32 {
            return Err(Error::InvalidId);
        }
        Ok(IdHex(s))
    }

    /// As &str
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Into String
    pub fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<&str> for IdHex {
    type Error = Error;

    fn try_from(s: &str) -> Result<IdHex, Error> {
        IdHex::try_from_str(s)
    }
}

impl From<Id> for IdHex {
    fn from(i: Id) -> IdHex {
        IdHex(i.as_hex_string())
    }
}

impl From<IdHex> for Id {
    fn from(h: IdHex) -> Id {
        // could only fail if IdHex is invalid
        Id::try_from_hex_string(&h.0).unwrap()
    }
}

impl Serialize for IdHex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for IdHex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(IdHexVisitor)
    }
}

struct IdHexVisitor;

impl Visitor<'_> for IdHexVisitor {
    type Value = IdHex;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a lowercase hexadecimal string representing 32 bytes")
    }

    fn visit_str<E>(self, v: &str) -> Result<IdHex, E>
    where
        E: serde::de::Error,
    {
        if v.len() != 64 {
            return Err(serde::de::Error::custom("IdHex is not 64 characters long"));
        }

        let vec: Vec<u8> = hex::decode(v).map_err(|e| serde::de::Error::custom(format!("{e}")))?;
        if vec.len() != 32 {
            return Err(serde::de::Error::custom("Invalid IdHex"));
        }

        Ok(IdHex(v.to_owned()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    test_serde! {Id, test_id_serde}
    test_serde! {IdHex, test_id_hex_serde}

    #[test]
    fn test_id_bech32() {
        let bech32 = Id::mock().as_bech32_string();
        println!("{bech32}");
        assert_eq!(Id::mock(), Id::try_from_bech32_string(&bech32).unwrap());
    }
}
