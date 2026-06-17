use crate::{Error, Event};
use derive_more::{AsMut, AsRef, Deref, Display, From, FromStr, Into};
use serde::{Deserialize, Serialize};
#[cfg(feature = "speedy")]
use speedy::{Context, Readable, Reader, Writable, Writer};

/// A Schnorr signature that signs an Event, taken on the Event Id field
#[derive(
    AsMut, AsRef, Clone, Copy, Debug, Deref, Eq, From, Into, PartialEq, Serialize, Deserialize,
)]
pub struct Signature(pub secp256k1::schnorr::Signature);

impl Signature {
    /// Render into a hexadecimal string
    pub fn as_hex_string(&self) -> String {
        hex::encode(self.0.as_ref())
    }

    /// Create from a hexadecimal string
    pub fn try_from_hex_string(v: &str) -> Result<Signature, Error> {
        let vec: Vec<u8> = hex::decode(v)?;
        Ok(Signature(secp256k1::schnorr::Signature::from_byte_array(
            vec.try_into().map_err(|_| Error::WrongLength)?,
        )))
    }

    /// A dummy signature of all zeroes
    pub fn zeroes() -> Signature {
        Signature(secp256k1::schnorr::Signature::from_byte_array([0; 64]))
    }

    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) async fn mock() -> Signature {
        let event = Event::mock().await;
        event.sig
    }
}

#[cfg(feature = "speedy")]
impl<'a, C: Context> Readable<'a, C> for Signature {
    #[inline]
    fn read_from<R: Reader<'a, C>>(reader: &mut R) -> Result<Self, C::Error> {
        let bytes: Vec<u8> = reader.read_vec(64)?;
        let sig =
            secp256k1::schnorr::Signature::from_slice(&bytes[..]).map_err(speedy::Error::custom)?;
        Ok(Signature(sig))
    }

    #[inline]
    fn minimum_bytes_needed() -> usize {
        64
    }
}

#[cfg(feature = "speedy")]
impl<C: Context> Writable<C> for Signature {
    #[inline]
    fn write_to<T: ?Sized + Writer<C>>(&self, writer: &mut T) -> Result<(), C::Error> {
        let bytes = self.0.as_ref();
        assert_eq!(bytes.as_slice().len(), 64);
        writer.write_bytes(bytes.as_slice())
    }

    #[inline]
    fn bytes_needed(&self) -> Result<usize, C::Error> {
        Ok(64)
    }
}

/// A Schnorr signature that signs an Event, taken on the Event Id field, as a hex string
#[derive(
    AsMut,
    AsRef,
    Clone,
    Debug,
    Deref,
    Deserialize,
    Display,
    Eq,
    From,
    FromStr,
    Hash,
    Into,
    PartialEq,
    Serialize,
)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct SignatureHex(pub String);

impl SignatureHex {
    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) async fn mock() -> SignatureHex {
        From::from(Signature::mock().await)
    }
}

impl From<Signature> for SignatureHex {
    fn from(s: Signature) -> SignatureHex {
        SignatureHex(s.as_hex_string())
    }
}

impl TryFrom<SignatureHex> for Signature {
    type Error = Error;

    fn try_from(sh: SignatureHex) -> Result<Signature, Error> {
        Signature::try_from_hex_string(&sh.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    test_serde_async! {Signature, test_signature_serde}

    #[cfg(feature = "speedy")]
    #[tokio::test]
    async fn test_speedy_signature() {
        let sig = Signature::mock().await;
        let bytes = sig.write_to_vec().unwrap();
        let sig2 = Signature::read_from_buffer(&bytes).unwrap();
        assert_eq!(sig, sig2);
    }
}
