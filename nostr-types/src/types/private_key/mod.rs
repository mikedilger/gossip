use crate::{
    Error, Event, Id, MutExportableSigner, PreEvent, PublicKey, Signature, Signer, SignerExt,
};
use async_trait::async_trait;
use std::convert::TryFrom;
use std::fmt;

mod encrypted_private_key;
pub use encrypted_private_key::*;

mod content_encryption;
pub use content_encryption::*;

/// This indicates the security of the key by keeping track of whether the
/// secret key material was handled carefully. If the secret is exposed in any
/// way, or leaked and the memory not zeroed, the key security drops to Weak.
///
/// This is a Best Effort tag. There are ways to leak the key and still have this
/// tag claim the key is Medium security. So Medium really means it might not
/// have leaked, whereas Weak means we know that it definately did leak.
///
/// We offer no Strong security via the PrivateKey structure. If we support
/// hardware tokens in the future, it will probably be via a different structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum KeySecurity {
    /// This means that the key was exposed in a way such that this library
    /// cannot ensure it's secrecy, usually either by being exported as a hex string,
    /// or by being imported from the same. Often in these cases it is displayed
    /// on the screen or left in the cut buffer or in freed memory that was not
    /// subsequently zeroed.
    Weak = 0,

    /// This means that the key might not have been directly exposed. But it still
    /// might have as there are numerous ways you can leak it such as exporting it
    /// and then decrypting the exported key, using unsafe rust, transmuting it into
    /// a different type that doesn't protect it, or using a privileged process to
    /// scan memory. Additionally, more advanced techniques can get at your key such
    /// as hardware attacks like spectre, rowhammer, and power analysis.
    Medium = 1,

    /// Not tracked
    NotTracked = 2,
}

impl TryFrom<u8> for KeySecurity {
    type Error = Error;

    fn try_from(i: u8) -> Result<KeySecurity, Error> {
        if i == 0 {
            Ok(KeySecurity::Weak)
        } else if i == 1 {
            Ok(KeySecurity::Medium)
        } else if i == 2 {
            Ok(KeySecurity::NotTracked)
        } else {
            Err(Error::UnknownKeySecurity(i))
        }
    }
}

/// This is a private key which is to be kept secret and is used to prove identity
#[allow(missing_debug_implementations)]
#[derive(Clone)]
pub struct PrivateKey(secp256k1::SecretKey, KeySecurity);

impl Default for PrivateKey {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PRIVATE-KEY-ELIDED")
    }
}

impl PrivateKey {
    /// Generate a new `PrivateKey` (which can be used to get the `PublicKey`)
    #[inline]
    pub fn new() -> PrivateKey {
        Self::generate()
    }

    /// Generate a new `PrivateKey` (which can be used to get the `PublicKey`)
    pub fn generate() -> PrivateKey {
        let mut secret_key;
        loop {
            secret_key = secp256k1::SecretKey::new(&mut rand::rng());
            let (_, parity) = secret_key.x_only_public_key(secp256k1::SECP256K1);
            if parity == secp256k1::Parity::Even {
                break;
            }
        }

        PrivateKey(secret_key, KeySecurity::Medium)
    }

    /// Get the PublicKey matching this PrivateKey
    pub fn public_key(&self) -> PublicKey {
        let (xopk, _parity) = self.0.x_only_public_key(secp256k1::SECP256K1);
        PublicKey::from_bytes(&xopk.serialize(), false).unwrap()
    }

    /// Get the security level of the private key
    pub fn key_security(&self) -> KeySecurity {
        self.1
    }

    /// Render into a hexadecimal string
    ///
    /// WARNING: This weakens the security of your key. Your key will be marked
    /// with `KeySecurity::Weak` if you execute this.
    pub fn as_hex_string(&mut self) -> String {
        self.1 = KeySecurity::Weak;
        hex::encode(self.0.secret_bytes())
    }

    /// Create from a hexadecimal string
    ///
    /// This creates a key with `KeySecurity::Weak`.  Use `generate()` or
    /// `import_encrypted()` for `KeySecurity::Medium`
    pub fn try_from_hex_string(v: &str) -> Result<PrivateKey, Error> {
        let vec: Vec<u8> = hex::decode(v)?;
        Ok(PrivateKey(
            secp256k1::SecretKey::from_byte_array(vec.try_into().map_err(|_| Error::WrongLength)?)?,
            KeySecurity::Weak,
        ))
    }

    /// Export as a bech32 encoded string
    ///
    /// WARNING: This weakens the security of your key. Your key will be marked
    /// with `KeySecurity::Weak` if you execute this.
    pub fn as_bech32_string(&mut self) -> String {
        self.1 = KeySecurity::Weak;
        bech32::encode::<bech32::Bech32>(*crate::HRP_NSEC, self.0.secret_bytes().as_slice())
            .unwrap()
    }

    /// Import from a bech32 encoded string
    ///
    /// This creates a key with `KeySecurity::Weak`.  Use `generate()` or
    /// `import_encrypted()` for `KeySecurity::Medium`
    pub fn try_from_bech32_string(s: &str) -> Result<PrivateKey, Error> {
        let data = bech32::decode(s)?;
        if data.0 != *crate::HRP_NSEC {
            Err(Error::WrongBech32(
                crate::HRP_NSEC.to_lowercase(),
                data.0.to_lowercase(),
            ))
        } else {
            Ok(PrivateKey(
                secp256k1::SecretKey::from_byte_array(
                    data.1.try_into().map_err(|_| Error::WrongLength)?,
                )?,
                KeySecurity::Weak,
            ))
        }
    }

    /// As a `secp256k1::SecretKey`
    pub fn as_secret_key(&self) -> secp256k1::SecretKey {
        self.0
    }

    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) fn mock() -> PrivateKey {
        PrivateKey::generate()
    }
}

impl Drop for PrivateKey {
    fn drop(&mut self) {
        self.0.non_secure_erase();
    }
}

#[async_trait]
impl Signer for PrivateKey {
    fn public_key(&self) -> PublicKey {
        self.public_key()
    }

    fn encrypted_private_key(&self) -> Option<EncryptedPrivateKey> {
        None
    }

    async fn sign_event(&self, input: PreEvent) -> Result<Event, Error> {
        // Verify the pubkey matches
        if input.pubkey != self.public_key() {
            return Err(Error::InvalidPrivateKey);
        }

        // Generate Id
        let id = input.hash()?;

        // Generate Signature
        let signature = self.sign_id(id).await?;

        Ok(Event {
            id,
            pubkey: input.pubkey,
            created_at: input.created_at,
            kind: input.kind,
            tags: input.tags,
            content: input.content,
            sig: signature,
        })
    }

    async fn encrypt(
        &self,
        other: &PublicKey,
        plaintext: &str,
        algo: ContentEncryptionAlgorithm,
    ) -> Result<String, Error> {
        self.encrypt(other, plaintext, algo)
    }

    /// Decrypt NIP-44
    async fn decrypt(&self, other: &PublicKey, ciphertext: &str) -> Result<String, Error> {
        self.decrypt(other, ciphertext)
    }

    fn key_security(&self) -> Result<KeySecurity, Error> {
        Ok(KeySecurity::NotTracked)
    }
}

#[async_trait]
impl SignerExt for PrivateKey {
    async fn sign_id(&self, id: Id) -> Result<Signature, Error> {
        let keypair = secp256k1::Keypair::from_secret_key(secp256k1::SECP256K1, &self.0);
        let message = secp256k1::Message::from_digest(id.0);
        Ok(Signature(keypair.sign_schnorr(message.as_ref().as_slice())))
    }

    async fn sign(&self, message: &[u8]) -> Result<Signature, Error> {
        use secp256k1::hashes::{sha256, Hash};
        let keypair = secp256k1::Keypair::from_secret_key(secp256k1::SECP256K1, &self.0);
        let hash = sha256::Hash::hash(message).to_byte_array();
        let message = secp256k1::Message::from_digest(hash);
        Ok(Signature(keypair.sign_schnorr(message.as_ref().as_slice())))
    }

    async fn nip44_conversation_key(&self, other: &PublicKey) -> Result<[u8; 32], Error> {
        Ok(nip44::get_conversation_key(
            self.0,
            other.as_xonly_public_key(),
        ))
    }
}

#[async_trait]
impl MutExportableSigner for PrivateKey {
    async fn export_private_key_in_hex(
        &mut self,
        _pass: &str,
        _log_n: u8,
    ) -> Result<(String, bool), Error> {
        Ok((self.as_hex_string(), false))
    }

    async fn export_private_key_in_bech32(
        &mut self,
        _pass: &str,
        _log_n: u8,
    ) -> Result<(String, bool), Error> {
        Ok((self.as_bech32_string(), false))
    }
}

fn base64flex() -> base64::engine::GeneralPurpose {
    let config = base64::engine::GeneralPurposeConfig::new()
        .with_decode_allow_trailing_bits(true)
        .with_encode_padding(true)
        .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent);
    base64::engine::GeneralPurpose::new(&base64::alphabet::STANDARD, config)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_privkey_bech32() {
        let mut pk = PrivateKey::mock();

        let encoded = pk.as_bech32_string();
        println!("bech32: {encoded}");

        let decoded = PrivateKey::try_from_bech32_string(&encoded).unwrap();

        assert_eq!(pk.0.secret_bytes(), decoded.0.secret_bytes());
        assert_eq!(decoded.1, KeySecurity::Weak);
    }
}
