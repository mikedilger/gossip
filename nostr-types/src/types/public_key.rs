use crate::{Error, PrivateKey, Signature};
use derive_more::{AsMut, AsRef, Deref, Display, From, FromStr, Into};
pub use secp256k1::XOnlyPublicKey;
use secp256k1::SECP256K1;
use serde::de::{Deserializer, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
#[cfg(feature = "speedy")]
use speedy::{Context, Readable, Reader, Writable, Writer};
use std::fmt;

/// This is a public key, which identifies an actor (usually a person) and is shared.
#[derive(AsMut, AsRef, Copy, Clone, Debug, Deref, Eq, From, Into, PartialEq, PartialOrd, Ord)]
pub struct PublicKey([u8; 32]);

impl PublicKey {
    /// Render into a hexadecimal string
    ///
    /// Consider converting `.into()` a `PublicKeyHex` which is a wrapped type rather than a naked `String`
    pub fn as_hex_string(&self) -> String {
        hex::encode(self.0)
    }

    /// Create from a hexadecimal string
    ///
    /// If verify is true, will verify that it works as a XOnlyPublicKey. This
    /// has a performance cost.
    pub fn try_from_hex_string(v: &str, verify: bool) -> Result<PublicKey, Error> {
        let vec: Vec<u8> = hex::decode(v)?;
        // if it's not 32 bytes, dont even try
        if vec.len() != 32 {
            Err(Error::InvalidPublicKey)
        } else {
            let array: [u8; 32] = vec.try_into().unwrap();
            if verify {
                let _ = XOnlyPublicKey::from_byte_array(array)?;
            }
            Ok(PublicKey(array))
        }
    }

    /// Export as a bech32 encoded string
    pub fn as_bech32_string(&self) -> String {
        bech32::encode::<bech32::Bech32>(*crate::HRP_NPUB, self.0.as_slice()).unwrap()
    }

    /// Export as XOnlyPublicKey
    pub fn as_xonly_public_key(&self) -> XOnlyPublicKey {
        XOnlyPublicKey::from_byte_array(self.0).unwrap()
    }

    /// Import from a bech32 encoded string
    ///
    /// If verify is true, will verify that it works as a XOnlyPublicKey. This
    /// has a performance cost.
    pub fn try_from_bech32_string(s: &str, verify: bool) -> Result<PublicKey, Error> {
        let data = bech32::decode(s)?;
        if data.0 != *crate::HRP_NPUB {
            Err(Error::WrongBech32(
                crate::HRP_NPUB.to_lowercase(),
                data.0.to_lowercase(),
            ))
        } else if data.1.len() != 32 {
            Err(Error::InvalidPublicKey)
        } else {
            let array: [u8; 32] = data.1.try_into().unwrap();
            if verify {
                let _ = XOnlyPublicKey::from_byte_array(array)?;
            }
            Ok(PublicKey(array))
        }
    }

    /// Import from raw bytes
    pub fn from_bytes(bytes: &[u8], verify: bool) -> Result<PublicKey, Error> {
        if bytes.len() != 32 {
            Err(Error::InvalidPublicKey)
        } else {
            if verify {
                let _ = XOnlyPublicKey::from_byte_array(bytes.try_into().unwrap())?;
            }
            Ok(PublicKey(bytes.try_into().unwrap()))
        }
    }

    /// Export as raw bytes
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    /// Export as raw bytes
    #[inline]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.0.as_slice().to_vec()
    }

    /// Verify a signed message
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<(), Error> {
        use secp256k1::hashes::{sha256, Hash};
        let pk = XOnlyPublicKey::from_byte_array(self.0)?;
        let hash = sha256::Hash::hash(message).to_byte_array();
        let message = secp256k1::Message::from_digest(hash);
        Ok(SECP256K1.verify_schnorr(&signature.0, message.as_ref().as_slice(), &pk)?)
    }

    /// This provides a u32 hash input for HyperLogLog that is based on
    /// NIP-45 (pr #1561).  This works with the original HyperLogLog
    pub fn hyperloglog_hash32(&self, offset: usize) -> u32 {
        if offset + 3 >= 32 {
            panic!("hyperloglog_hash32 offset cannot be greater than 28");
        }
        u32::from_ne_bytes(self.0[offset..offset + 3].try_into().unwrap())
    }

    /// This provides a u64 hash input for HyperLogLog++ that is based on
    /// NIP-45 (pr #1561).  This works with Google's HyperLogLog++
    pub fn hyperloglog_hash64(&self, offset: usize) -> u64 {
        if offset + 7 >= 32 {
            panic!("hyperloglog_hash64 offset cannot be greater than 24");
        }
        u64::from_ne_bytes(self.0[offset..offset + 7].try_into().unwrap())
    }

    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) fn mock() -> PublicKey {
        PrivateKey::generate().public_key()
    }

    #[allow(dead_code)]
    pub(crate) fn mock_deterministic() -> PublicKey {
        PublicKey::try_from_hex_string(
            "ee11a5dff40c19a555f41fe42b48f00e618c91225622ae37b6c2bb67b76c4e49",
            true,
        )
        .unwrap()
    }
}

impl Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.as_hex_string())
    }
}

impl<'de> Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(PublicKeyVisitor)
    }
}

struct PublicKeyVisitor;

impl Visitor<'_> for PublicKeyVisitor {
    type Value = PublicKey;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a lowercase hexadecimal string representing 32 bytes")
    }

    fn visit_str<E>(self, v: &str) -> Result<PublicKey, E>
    where
        E: serde::de::Error,
    {
        let vec: Vec<u8> = hex::decode(v).map_err(|e| serde::de::Error::custom(format!("{e}")))?;

        if vec.len() != 32 {
            return Err(serde::de::Error::custom("Public key is not 32 bytes long"));
        }

        Ok(PublicKey(vec.try_into().unwrap()))
    }
}

impl std::hash::Hash for PublicKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_hex_string().hash(state);
    }
}

#[cfg(feature = "speedy")]
impl<'a, C: Context> Readable<'a, C> for PublicKey {
    #[inline]
    fn read_from<R: Reader<'a, C>>(reader: &mut R) -> Result<Self, C::Error> {
        let bytes: Vec<u8> = reader.read_vec(32)?;
        Ok(PublicKey(bytes.try_into().unwrap()))
    }

    #[inline]
    fn minimum_bytes_needed() -> usize {
        32
    }
}

#[cfg(feature = "speedy")]
impl<C: Context> Writable<C> for PublicKey {
    #[inline]
    fn write_to<T: ?Sized + Writer<C>>(&self, writer: &mut T) -> Result<(), C::Error> {
        writer.write_bytes(self.as_slice())
    }

    #[inline]
    fn bytes_needed(&self) -> Result<usize, C::Error> {
        Ok(32)
    }
}

/// This is a public key, which identifies an actor (usually a person) and is shared, as a hex string
///
/// You can convert from a `PublicKey` into this with `From`/`Into`.  You can convert this back to a `PublicKey` with `TryFrom`/`TryInto`.
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
    PartialEq,
    PartialOrd,
    Ord,
)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct PublicKeyHex(String);

impl PublicKeyHex {
    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) fn mock() -> PublicKeyHex {
        From::from(PublicKey::mock())
    }

    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) fn mock_deterministic() -> PublicKeyHex {
        PublicKey::mock_deterministic().into()
    }

    /// Export as a bech32 encoded string
    pub fn as_bech32_string(&self) -> String {
        let vec: Vec<u8> = hex::decode(&self.0).unwrap();
        bech32::encode::<bech32::Bech32>(*crate::HRP_NPUB, &vec).unwrap()
    }

    /// Try from &str
    pub fn try_from_str(s: &str) -> Result<PublicKeyHex, Error> {
        Self::try_from_string(s.to_owned())
    }

    /// Try from String
    pub fn try_from_string(s: String) -> Result<PublicKeyHex, Error> {
        if s.len() != 64 {
            return Err(Error::InvalidPublicKey);
        }
        let vec: Vec<u8> = hex::decode(&s)?;
        if vec.len() != 32 {
            return Err(Error::InvalidPublicKey);
        }
        Ok(PublicKeyHex(s))
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

impl TryFrom<&str> for PublicKeyHex {
    type Error = Error;

    fn try_from(s: &str) -> Result<PublicKeyHex, Error> {
        PublicKeyHex::try_from_str(s)
    }
}

impl From<&PublicKey> for PublicKeyHex {
    fn from(pk: &PublicKey) -> PublicKeyHex {
        PublicKeyHex(pk.as_hex_string())
    }
}

impl From<PublicKey> for PublicKeyHex {
    fn from(pk: PublicKey) -> PublicKeyHex {
        PublicKeyHex(pk.as_hex_string())
    }
}

impl TryFrom<&PublicKeyHex> for PublicKey {
    type Error = Error;

    fn try_from(pkh: &PublicKeyHex) -> Result<PublicKey, Error> {
        PublicKey::try_from_hex_string(&pkh.0, true)
    }
}

impl TryFrom<PublicKeyHex> for PublicKey {
    type Error = Error;

    fn try_from(pkh: PublicKeyHex) -> Result<PublicKey, Error> {
        PublicKey::try_from_hex_string(&pkh.0, true)
    }
}

impl Serialize for PublicKeyHex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for PublicKeyHex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(PublicKeyHexVisitor)
    }
}

struct PublicKeyHexVisitor;

impl Visitor<'_> for PublicKeyHexVisitor {
    type Value = PublicKeyHex;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a lowercase hexadecimal string representing 32 bytes")
    }

    fn visit_str<E>(self, v: &str) -> Result<PublicKeyHex, E>
    where
        E: serde::de::Error,
    {
        if v.len() != 64 {
            return Err(serde::de::Error::custom(
                "PublicKeyHex is not 64 characters long",
            ));
        }

        let vec: Vec<u8> = hex::decode(v).map_err(|e| serde::de::Error::custom(format!("{e}")))?;
        if vec.len() != 32 {
            return Err(serde::de::Error::custom("Invalid PublicKeyHex"));
        }

        Ok(PublicKeyHex(v.to_owned()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    test_serde! {PublicKey, test_public_key_serde}
    test_serde! {PublicKeyHex, test_public_key_hex_serde}

    #[test]
    fn test_pubkey_bech32() {
        let pk = PublicKey::mock();

        let encoded = pk.as_bech32_string();
        println!("bech32: {encoded}");

        let decoded = PublicKey::try_from_bech32_string(&encoded, true).unwrap();

        assert_eq!(pk, decoded);
    }

    #[cfg(feature = "speedy")]
    #[test]
    fn test_speedy_public_key() {
        let pk = PublicKey::mock();
        let bytes = pk.write_to_vec().unwrap();
        let pk2 = PublicKey::read_from_buffer(&bytes).unwrap();
        assert_eq!(pk, pk2);
    }
}
