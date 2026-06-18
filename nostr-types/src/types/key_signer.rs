use crate::{
    ContentEncryptionAlgorithm, EncryptedPrivateKey, Error, Event, ExportableSigner, Id,
    KeySecurity, LockableSigner, PreEvent, PrivateKey, PublicKey, Signature, Signer, SignerExt,
};
use async_trait::async_trait;
use serde::de::Error as DeError;
use serde::de::{Deserialize, Deserializer, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeSeq, Serializer};
use std::fmt;
use std::sync::RwLock;

/// Signer with a local private key (and public key)
pub struct KeySigner {
    public_key: PublicKey,
    encrypted_private_key: RwLock<EncryptedPrivateKey>,
    private_key: RwLock<Option<PrivateKey>>,
}

impl fmt::Debug for KeySigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("KeySigner")
            .field(
                "encrypted_private_key",
                &*self.encrypted_private_key.read().unwrap(),
            )
            .field("public_key", &self.public_key)
            .finish()
    }
}

impl KeySigner {
    /// Create a Signer from an `EncryptedPrivateKey`
    pub fn from_locked_parts(epk: EncryptedPrivateKey, pk: PublicKey) -> Self {
        Self {
            public_key: pk,
            encrypted_private_key: RwLock::new(epk),
            private_key: RwLock::new(None),
        }
    }

    /// Create a Signer from a `PrivateKey`
    pub fn from_private_key(privk: PrivateKey, password: &str, log_n: u8) -> Result<Self, Error> {
        let epk = privk.export_encrypted(password, log_n)?;
        Ok(Self {
            public_key: privk.public_key(),
            encrypted_private_key: RwLock::new(epk),
            private_key: RwLock::new(Some(privk)),
        })
    }

    /// Create a Signer from an `EncryptedPrivateKey` and a password to unlock it
    pub fn from_encrypted_private_key(epk: EncryptedPrivateKey, pass: &str) -> Result<Self, Error> {
        let priv_key = epk.decrypt(pass)?;
        let pub_key = priv_key.public_key();
        Ok(Self {
            encrypted_private_key: RwLock::new(epk),
            public_key: pub_key,
            private_key: RwLock::new(Some(priv_key)),
        })
    }

    /// Create a Signer by generating a new `PrivateKey`
    pub fn generate(password: &str, log_n: u8) -> Result<Self, Error> {
        let privk = PrivateKey::generate();
        let epk = privk.export_encrypted(password, log_n)?;
        Ok(Self {
            public_key: privk.public_key(),
            encrypted_private_key: RwLock::new(epk),
            private_key: RwLock::new(Some(privk)),
        })
    }
}

#[async_trait]
impl Signer for KeySigner {
    fn public_key(&self) -> PublicKey {
        self.public_key
    }

    fn encrypted_private_key(&self) -> Option<EncryptedPrivateKey> {
        Some(self.encrypted_private_key.read().unwrap().clone())
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
        match &*self.private_key.read().unwrap() {
            Some(pk) => pk.encrypt(other, plaintext, algo),
            None => Err(Error::SignerIsLocked),
        }
    }

    async fn decrypt(&self, other: &PublicKey, ciphertext: &str) -> Result<String, Error> {
        match &*self.private_key.read().unwrap() {
            Some(pk) => pk.decrypt(other, ciphertext),
            None => Err(Error::SignerIsLocked),
        }
    }

    fn key_security(&self) -> Result<KeySecurity, Error> {
        match &*self.private_key.read().unwrap() {
            Some(pk) => Ok(pk.key_security()),
            None => Err(Error::SignerIsLocked),
        }
    }
}

#[async_trait]
impl SignerExt for KeySigner {
    async fn sign_id(&self, id: Id) -> Result<Signature, Error> {
        let Some(pk) = self.private_key.read().unwrap().clone() else {
            return Err(Error::SignerIsLocked);
        };
        pk.sign_id(id).await
    }

    async fn sign(&self, message: &[u8]) -> Result<Signature, Error> {
        let Some(pk) = self.private_key.read().unwrap().clone() else {
            return Err(Error::SignerIsLocked);
        };
        pk.sign(message).await
    }

    async fn nip44_conversation_key(&self, other: &PublicKey) -> Result<[u8; 32], Error> {
        let xpub = other.as_xonly_public_key();
        match &*self.private_key.read().unwrap() {
            Some(pk) => Ok(nip44::get_conversation_key(pk.as_secret_key(), xpub)),
            None => Err(Error::SignerIsLocked),
        }
    }
}

impl LockableSigner for KeySigner {
    fn is_locked(&self) -> bool {
        self.private_key.read().unwrap().is_none()
    }

    fn unlock(&self, password: &str) -> Result<(), Error> {
        if !self.is_locked() {
            return Ok(());
        }

        let private_key = self
            .encrypted_private_key
            .read()
            .unwrap()
            .decrypt(password)?;

        *self.private_key.write().unwrap() = Some(private_key);

        Ok(())
    }

    fn lock(&self) {
        *self.private_key.write().unwrap() = None;
    }

    fn change_passphrase(&self, old: &str, new: &str, log_n: u8) -> Result<(), Error> {
        let private_key = self.encrypted_private_key.read().unwrap().decrypt(old)?;
        *self.encrypted_private_key.write().unwrap() = private_key.export_encrypted(new, log_n)?;
        *self.private_key.write().unwrap() = Some(private_key);
        Ok(())
    }

    fn upgrade(&self, pass: &str, log_n: u8) -> Result<(), Error> {
        let private_key = self.encrypted_private_key.read().unwrap().decrypt(pass)?;
        *self.encrypted_private_key.write().unwrap() = private_key.export_encrypted(pass, log_n)?;
        Ok(())
    }
}

#[async_trait]
impl ExportableSigner for KeySigner {
    async fn export_private_key_in_hex(
        &self,
        pass: &str,
        log_n: u8,
    ) -> Result<(String, bool), Error> {
        if let Some(ref mut pk) = *self.private_key.write().unwrap() {
            // Test password and check key security
            let pkcheck = self.encrypted_private_key.read().unwrap().decrypt(pass)?;

            // side effect: this may downgrade the key security of self.private_key
            let output = pk.as_hex_string();

            // If key security changed, re-export
            let mut downgraded = false;
            if pk.key_security() != pkcheck.key_security() {
                downgraded = true;
                *self.encrypted_private_key.write().unwrap() = pk.export_encrypted(pass, log_n)?;
            }
            Ok((output, downgraded))
        } else {
            Err(Error::SignerIsLocked)
        }
    }

    async fn export_private_key_in_bech32(
        &self,
        pass: &str,
        log_n: u8,
    ) -> Result<(String, bool), Error> {
        if let Some(ref mut pk) = *self.private_key.write().unwrap() {
            // Test password and check key security
            let pkcheck = self.encrypted_private_key.read().unwrap().decrypt(pass)?;

            // side effect: this may downgrade the key security of self.private_key
            let output = pk.as_bech32_string();

            // If key security changed, re-export
            let mut downgraded = false;
            if pk.key_security() != pkcheck.key_security() {
                downgraded = true;
                *self.encrypted_private_key.write().unwrap() = pk.export_encrypted(pass, log_n)?;
            }

            Ok((output, downgraded))
        } else {
            Err(Error::SignerIsLocked)
        }
    }
}

impl Serialize for KeySigner {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(&self.public_key)?;
        seq.serialize_element(&*self.encrypted_private_key.read().unwrap())?;
        seq.end()
    }
}

impl<'de> Deserialize<'de> for KeySigner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(KeySignerVisitor)
    }
}

struct KeySignerVisitor;

impl<'de> Visitor<'de> for KeySignerVisitor {
    type Value = KeySigner;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a key signer structure as a sequence")
    }

    fn visit_seq<A>(self, mut access: A) -> Result<KeySigner, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let public_key = access
            .next_element::<PublicKey>()?
            .ok_or_else(|| DeError::custom("Missing or invalid pubkey"))?;
        let epk = access
            .next_element::<EncryptedPrivateKey>()?
            .ok_or_else(|| DeError::custom("Missing or invalid epk"))?;

        Ok(KeySigner {
            public_key,
            encrypted_private_key: RwLock::new(epk),
            private_key: RwLock::new(None),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_key_signer_serde() {
        let ks = KeySigner::generate("password", 16).unwrap();
        let s = serde_json::to_string(&ks).unwrap();
        println!("{s}");
        let ks2: KeySigner = serde_json::from_str(&s).unwrap();
        assert_eq!(ks.public_key, ks2.public_key);
        assert_eq!(
            *ks.encrypted_private_key.read().unwrap(),
            *ks2.encrypted_private_key.read().unwrap()
        );
    }
}
