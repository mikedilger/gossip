#[cfg(feature = "nip46")]
use crate::nip46::BunkerClient;
use crate::{
    ContentEncryptionAlgorithm, DelegationConditions, EncryptedPrivateKey, Error, Event, Id,
    KeySecurity, KeySigner, LockableSigner, Metadata, PreEvent, PrivateKey, PublicKey, Rumor,
    Signature, Signer, SignerExt,
};
use serde::{Deserialize, Serialize};
use std::sync::mpsc::Sender;
use std::sync::Arc;

/// All states that your identity can be in
#[derive(Debug, Default, Serialize, Deserialize)]
pub enum Identity {
    /// No identity information
    #[default]
    None,

    /// Public key only
    Public(PublicKey),

    /// Private key
    Private(Arc<KeySigner>),

    /// Remote Signer (Bunker)
    #[cfg(feature = "nip46")]
    Remote(BunkerClient),
}

// No one besides the Identity has the internal Signer, so we can safely Send
unsafe impl Send for Identity {}

// Nobody can write while someone else is reading with just a non-mutable &reference
unsafe impl Sync for Identity {}

impl Identity {
    /// New `Identity` from a public key
    pub fn from_public_key(pk: PublicKey) -> Self {
        Self::Public(pk)
    }

    /// New `Identity` from a private key
    pub fn from_private_key(pk: PrivateKey, pass: &str, log_n: u8) -> Result<Self, Error> {
        let key_signer = KeySigner::from_private_key(pk, pass, log_n)?;
        Ok(Self::Private(Arc::new(key_signer)))
    }

    /// New `Identity` from an encrypted private key and a public key
    pub fn from_locked_parts(pk: PublicKey, epk: EncryptedPrivateKey) -> Self {
        let key_signer = KeySigner::from_locked_parts(epk, pk);
        Self::Private(Arc::new(key_signer))
    }

    /// New `Identity` from an encrypted private key and its password
    pub fn from_encrypted_private_key(epk: EncryptedPrivateKey, pass: &str) -> Result<Self, Error> {
        let key_signer = KeySigner::from_encrypted_private_key(epk, pass)?;
        Ok(Self::Private(Arc::new(key_signer)))
    }

    /// Generate a new `Identity`
    pub fn generate(password: &str, log_n: u8) -> Result<Self, Error> {
        let key_signer = KeySigner::generate(password, log_n)?;
        Ok(Self::Private(Arc::new(key_signer)))
    }

    /// Get access to the inner `KeySigner`
    pub fn inner_key_signer(&self) -> Option<Arc<KeySigner>> {
        match self {
            Self::None => None,
            Self::Public(_) => None,
            Self::Private(b) => Some(b.clone()),
            #[cfg(feature = "nip46")]
            Self::Remote(_) => None,
        }
    }

    /// Unlock
    pub fn unlock(&self, password: &str) -> Result<(), Error> {
        match self {
            Self::Private(arcsigner) => arcsigner.unlock(password),
            #[cfg(feature = "nip46")]
            Self::Remote(bc) => bc.unlock(password),
            _ => Ok(()),
        }
    }

    /// Lock access to the private key
    pub fn lock(&self) {
        match self {
            Self::Private(arcsigner) => arcsigner.lock(),
            #[cfg(feature = "nip46")]
            Self::Remote(bc) => bc.lock(),
            _ => (),
        }
    }

    /// Has a public key
    pub fn has_public_key(&self) -> bool {
        !matches!(self, Self::None)
    }

    /// Has a private key
    pub fn has_private_key(&self) -> bool {
        matches!(self, Self::Private(_))
    }

    /// Is the identity locked?
    pub fn is_locked(&self) -> bool {
        match self {
            Self::Private(box_signer) => box_signer.is_locked(),
            #[cfg(feature = "nip46")]
            Self::Remote(bc) => bc.is_locked(),
            _ => false,
        }
    }

    /// Is the identity unlocked?
    pub fn is_unlocked(&self) -> bool {
        // Not quite this: !self.is_locked()
        // Instead we want to know if it is usable
        // So None and Public are NOT unlocked and NOT locked either.

        match self {
            Self::Private(box_signer) => !box_signer.is_locked(),
            #[cfg(feature = "nip46")]
            Self::Remote(bc) => !bc.is_locked(),
            _ => false,
        }
    }

    /// Can sign if unlocked
    pub fn can_sign_if_unlocked(&self) -> bool {
        match self {
            Self::Private(_) => true,
            #[cfg(feature = "nip46")]
            Self::Remote(_) => true,
            _ => false,
        }
    }

    /// Change the passphrase used for locking access to the private key
    pub fn change_passphrase(&self, old: &str, new: &str, log_n: u8) -> Result<(), Error> {
        match self {
            Self::None => Err(Error::NoPublicKey),
            Self::Public(_) => Err(Error::NoPrivateKey),
            Self::Private(arcsigner) => arcsigner.change_passphrase(old, new, log_n),
            #[cfg(feature = "nip46")]
            Self::Remote(bunkerclient) => bunkerclient.change_passphrase(old, new, log_n),
        }
    }

    /// What is the public key?
    pub fn public_key(&self) -> Option<PublicKey> {
        match self {
            Self::None => None,
            Self::Public(pk) => Some(*pk),
            Self::Private(arcsigner) => Some(arcsigner.public_key()),
            #[cfg(feature = "nip46")]
            Self::Remote(bunkerclient) => Some(bunkerclient.public_key()),
        }
    }

    /// What is the signer's encrypted private key?
    pub fn encrypted_private_key(&self) -> Option<EncryptedPrivateKey> {
        if let Self::Private(arcsigner) = self {
            arcsigner.encrypted_private_key()
        } else {
            None
        }
    }

    /// Sign a 32-bit hash
    pub async fn sign_id(&self, id: Id) -> Result<Signature, Error> {
        match self {
            Self::None => Err(Error::NoPublicKey),
            Self::Public(_) => Err(Error::NoPrivateKey),
            Self::Private(arcsigner) => arcsigner.sign_id(id).await,
            #[cfg(feature = "nip46")]
            Self::Remote(_bunkerclient) => Err(Error::NoPrivateKey),
        }
    }

    /// Sign a message (this hashes with SHA-256 first internally)
    pub async fn sign(&self, message: &[u8]) -> Result<Signature, Error> {
        match self {
            Self::None => Err(Error::NoPublicKey),
            Self::Public(_) => Err(Error::NoPrivateKey),
            Self::Private(arcsigner) => arcsigner.sign(message).await,
            #[cfg(feature = "nip46")]
            Self::Remote(_bunkerclient) => Err(Error::NoPrivateKey),
        }
    }

    /// Encrypt
    pub async fn encrypt(
        &self,
        other: &PublicKey,
        plaintext: &str,
        algo: ContentEncryptionAlgorithm,
    ) -> Result<String, Error> {
        match self {
            Self::None => Err(Error::NoPublicKey),
            Self::Public(_) => Err(Error::NoPrivateKey),
            Self::Private(arcsigner) => arcsigner.encrypt(other, plaintext, algo).await,
            #[cfg(feature = "nip46")]
            Self::Remote(bunkerclient) => bunkerclient.encrypt(other, plaintext, algo).await,
        }
    }

    /// Decrypt
    pub async fn decrypt(&self, other: &PublicKey, ciphertext: &str) -> Result<String, Error> {
        match self {
            Self::None => Err(Error::NoPublicKey),
            Self::Public(_) => Err(Error::NoPrivateKey),
            Self::Private(arcsigner) => arcsigner.decrypt(other, ciphertext).await,
            #[cfg(feature = "nip46")]
            Self::Remote(bunkerclient) => bunkerclient.decrypt(other, ciphertext).await,
        }
    }

    /// Get NIP-44 conversation key
    pub async fn nip44_conversation_key(&self, other: &PublicKey) -> Result<[u8; 32], Error> {
        match self {
            Self::None => Err(Error::NoPublicKey),
            Self::Public(_) => Err(Error::NoPrivateKey),
            Self::Private(arcsigner) => arcsigner.nip44_conversation_key(other).await,
            #[cfg(feature = "nip46")]
            Self::Remote(_bunkerclient) => Err(Error::NoPrivateKey),
        }
    }

    /// Get the security level of the private key
    pub fn key_security(&self) -> Result<KeySecurity, Error> {
        match self {
            Self::None => Err(Error::NoPublicKey),
            Self::Public(_) => Err(Error::NoPrivateKey),
            Self::Private(arcsigner) => arcsigner.key_security(),
            #[cfg(feature = "nip46")]
            Self::Remote(bunkerclient) => bunkerclient.key_security(),
        }
    }

    /// Upgrade the encrypted private key to the latest format
    pub fn upgrade(&self, pass: &str, log_n: u8) -> Result<(), Error> {
        match self {
            Self::None => Err(Error::NoPublicKey),
            Self::Public(_) => Err(Error::NoPrivateKey),
            Self::Private(arcsigner) => arcsigner.upgrade(pass, log_n),
            #[cfg(feature = "nip46")]
            Self::Remote(_bunkerclient) => Err(Error::NoPrivateKey),
        }
    }

    /// Create an event that sets Metadata
    pub async fn create_metadata_event(
        &self,
        input: PreEvent,
        metadata: Metadata,
    ) -> Result<Event, Error> {
        match self {
            Self::None => Err(Error::NoPublicKey),
            Self::Public(_) => Err(Error::NoPrivateKey),
            Self::Private(arcsigner) => arcsigner.create_metadata_event(input, metadata).await,
            #[cfg(feature = "nip46")]
            Self::Remote(bunkerclient) => bunkerclient.create_metadata_event(input, metadata).await,
        }
    }

    /// Create a ZapRequest event These events are not published to nostr, they are sent to a lnurl.
    pub async fn create_zap_request_event(
        &self,
        recipient_pubkey: PublicKey,
        zapped_event: Option<Id>,
        millisatoshis: u64,
        relays: Vec<String>,
        content: String,
    ) -> Result<Event, Error> {
        match self {
            Self::None => Err(Error::NoPublicKey),
            Self::Public(_) => Err(Error::NoPrivateKey),
            Self::Private(arcsigner) => {
                arcsigner
                    .create_zap_request_event(
                        recipient_pubkey,
                        zapped_event,
                        millisatoshis,
                        relays,
                        content,
                    )
                    .await
            }
            #[cfg(feature = "nip46")]
            Self::Remote(bunkerclient) => {
                bunkerclient
                    .create_zap_request_event(
                        recipient_pubkey,
                        zapped_event,
                        millisatoshis,
                        relays,
                        content,
                    )
                    .await
            }
        }
    }

    /// Decrypt the contents of an event
    pub async fn decrypt_event_contents(&self, event: &Event) -> Result<String, Error> {
        match self {
            Self::None => Err(Error::NoPublicKey),
            Self::Public(_) => Err(Error::NoPrivateKey),
            Self::Private(arcsigner) => arcsigner.decrypt_event_contents(event).await,
            #[cfg(feature = "nip46")]
            Self::Remote(bunkerclient) => bunkerclient.decrypt_event_contents(event).await,
        }
    }

    /// If a gift wrap event, unwrap and return the inner Rumor
    pub async fn unwrap_giftwrap(&self, event: &Event) -> Result<Rumor, Error> {
        match self {
            Self::None => Err(Error::NoPublicKey),
            Self::Public(_) => Err(Error::NoPrivateKey),
            Self::Private(arcsigner) => arcsigner.unwrap_giftwrap(event).await,
            #[cfg(feature = "nip46")]
            Self::Remote(bunkerclient) => bunkerclient.unwrap_giftwrap(event).await,
        }
    }

    /// Generate delegation signature
    pub async fn generate_delegation_signature(
        &self,
        delegated_pubkey: PublicKey,
        delegation_conditions: &DelegationConditions,
    ) -> Result<Signature, Error> {
        match self {
            Self::None => Err(Error::NoPublicKey),
            Self::Public(_) => Err(Error::NoPrivateKey),
            Self::Private(arcsigner) => {
                arcsigner
                    .generate_delegation_signature(delegated_pubkey, delegation_conditions)
                    .await
            }
            #[cfg(feature = "nip46")]
            Self::Remote(_bunkerclient) => Err(Error::NoPrivateKey),
        }
    }

    /// Giftwrap an event
    pub async fn giftwrap(&self, input: PreEvent, pubkey: PublicKey) -> Result<Event, Error> {
        match self {
            Self::None => Err(Error::NoPublicKey),
            Self::Public(_) => Err(Error::NoPrivateKey),
            Self::Private(arcsigner) => arcsigner.giftwrap(input, pubkey).await,
            #[cfg(feature = "nip46")]
            Self::Remote(bunkerclient) => bunkerclient.giftwrap(input, pubkey).await,
        }
    }

    /// Sign an event
    pub async fn sign_event(&self, input: PreEvent) -> Result<Event, Error> {
        match self {
            Self::None => Err(Error::NoPublicKey),
            Self::Public(_) => Err(Error::NoPrivateKey),
            Self::Private(arcsigner) => arcsigner.sign_event(input).await,
            #[cfg(feature = "nip46")]
            Self::Remote(bunkerclient) => bunkerclient.sign_event(input).await,
        }
    }

    /// Sign an event with Proof-of-Work
    pub async fn sign_event_with_pow(
        &self,
        input: PreEvent,
        zero_bits: u8,
        work_sender: Option<Sender<u8>>,
    ) -> Result<Event, Error> {
        match self {
            Self::None => Err(Error::NoPublicKey),
            Self::Public(_) => Err(Error::NoPrivateKey),
            Self::Private(arcsigner) => {
                arcsigner
                    .sign_event_with_pow(input, zero_bits, work_sender)
                    .await
            }
            #[cfg(feature = "nip46")]
            Self::Remote(_bunkerclient) => Err(Error::NoPrivateKey),
        }
    }

    /// Verify delegation signature
    pub fn verify_delegation_signature(
        &self,
        delegated_pubkey: PublicKey,
        delegation_conditions: &DelegationConditions,
        signature: &Signature,
    ) -> Result<(), Error> {
        match self {
            Self::None => Err(Error::NoPublicKey),
            Self::Public(_) => Err(Error::NoPrivateKey),
            Self::Private(arcsigner) => arcsigner.verify_delegation_signature(
                delegated_pubkey,
                delegation_conditions,
                signature,
            ),
            #[cfg(feature = "nip46")]
            Self::Remote(_bunkerclient) => Err(Error::NoPrivateKey),
        }
    }
}
