use crate::error::Error;
use nostr_types::{EncryptedPrivateKey, Event, KeySecurity, PreEvent, PrivateKey, PublicKey};

pub enum Signer {
    Fresh,
    Encrypted(EncryptedPrivateKey),
    Ready(PrivateKey),
}

impl Default for Signer {
    fn default() -> Signer {
        Signer::Fresh
    }
}

impl Signer {
    #[allow(dead_code)]
    pub fn load_encrypted_private_key(&mut self, epk: EncryptedPrivateKey) {
        *self = Signer::Encrypted(epk);
    }

    #[allow(dead_code)]
    pub fn unlock_encrypted_private_key(&mut self, pass: &str) -> Result<(), Error> {
        if let Signer::Encrypted(epk) = self {
            *self = Signer::Ready(epk.decrypt(pass)?);
            Ok(())
        } else {
            Err(Error::NoPrivateKey)
        }
    }

    #[allow(dead_code)]
    pub fn is_loaded(&self) -> bool {
        matches!(self, Signer::Encrypted(_)) || matches!(self, Signer::Ready(_))
    }

    #[allow(dead_code)]
    pub fn is_ready(&self) -> bool {
        matches!(self, Signer::Ready(_))
    }

    #[allow(dead_code)]
    pub fn public_key(&self) -> Option<PublicKey> {
        if let Signer::Ready(pk) = self {
            Some(pk.public_key())
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn key_security(&self) -> Option<KeySecurity> {
        if let Signer::Ready(pk) = self {
            Some(pk.key_security())
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn sign_preevent(&self, preevent: PreEvent) -> Result<Event, Error> {
        match self {
            Signer::Ready(pk) => Ok(Event::new(preevent, pk)?),
            _ => Err(Error::NoPrivateKey),
        }
    }
}
