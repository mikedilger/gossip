use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{EncryptedPrivateKey, Event, KeySecurity, PreEvent, PrivateKey, PublicKey};
use tokio::task;

pub enum Signer {
    Fresh,
    Encrypted(EncryptedPrivateKey),
    Ready(PrivateKey, EncryptedPrivateKey),
}

impl Default for Signer {
    fn default() -> Signer {
        Signer::Fresh
    }
}

impl Signer {
    pub fn load_encrypted_private_key(&mut self, epk: EncryptedPrivateKey) {
        *self = Signer::Encrypted(epk);
    }

    pub fn unlock_encrypted_private_key(&mut self, pass: &str) -> Result<(), Error> {
        if let Signer::Encrypted(epk) = self {
            *self = Signer::Ready(epk.decrypt(pass)?, epk.clone());
            Ok(())
        } else {
            Err(Error::NoPrivateKey)
        }
    }

    pub fn generate_private_key(&mut self, pass: &str) -> Result<EncryptedPrivateKey, Error> {
        let pk = PrivateKey::generate();
        let epk = pk.export_encrypted(pass)?;
        *self = Signer::Ready(pk, epk.clone());
        Ok(epk)
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self, Signer::Encrypted(_)) || matches!(self, Signer::Ready(_, _))
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Signer::Ready(_, _))
    }

    pub fn public_key(&self) -> Option<PublicKey> {
        if let Signer::Ready(pk, _) = self {
            Some(pk.public_key())
        } else {
            None
        }
    }

    pub fn encrypted_private_key(&self) -> Option<EncryptedPrivateKey> {
        if let Signer::Ready(_, epk) = self {
            Some(epk.clone())
        } else {
            None
        }
    }

    pub fn key_security(&self) -> Option<KeySecurity> {
        if let Signer::Ready(pk, _) = self {
            Some(pk.key_security())
        } else {
            None
        }
    }

    pub fn sign_preevent(&self, preevent: PreEvent, pow: Option<u8>) -> Result<Event, Error> {
        match self {
            Signer::Ready(pk, _) => match pow {
                Some(pow) => Ok(Event::new_with_pow(preevent, pk, pow)?),
                None => Ok(Event::new(preevent, pk)?),
            },
            _ => Err(Error::NoPrivateKey),
        }
    }

    pub fn export_private_key_bech32(&mut self, pass: &str) -> Result<String, Error> {
        match self {
            Signer::Ready(_, epk) => {
                let mut pk = epk.decrypt(pass)?;
                let output = pk.try_as_bech32_string()?;

                // We have to regenerate encrypted private key because it may have fallen from
                // medium to weak security.
                let epk = pk.export_encrypted(pass)?;

                // And then we have to save that
                let mut settings = GLOBALS.settings.blocking_write();
                settings.encrypted_private_key = Some(epk.clone());
                task::spawn(async move {
                    if let Err(e) = settings.save().await {
                        tracing::error!("{}", e);
                    }
                });

                *self = Signer::Ready(pk, epk);
                Ok(output)
            }
            _ => Err(Error::NoPrivateKey),
        }
    }

    pub fn export_private_key_hex(&mut self, pass: &str) -> Result<String, Error> {
        match self {
            Signer::Ready(_, epk) => {
                let mut pk = epk.decrypt(pass)?;
                let output = pk.as_hex_string();

                // We have to regenerate encrypted private key because it may have fallen from
                // medium to weak security.
                let epk = pk.export_encrypted(pass)?;

                // And then we have to save that
                let mut settings = GLOBALS.settings.blocking_write();
                settings.encrypted_private_key = Some(epk.clone());
                task::spawn(async move {
                    if let Err(e) = settings.save().await {
                        tracing::error!("{}", e);
                    }
                });

                *self = Signer::Ready(pk, epk);
                Ok(output)
            }
            _ => Err(Error::NoPrivateKey),
        }
    }

    pub fn delete_identity(&mut self, pass: &str) -> Result<(), Error> {
        match self {
            Signer::Ready(_, epk) => {
                // Verify their password
                let _pk = epk.decrypt(pass)?;

                // Delete from database
                let mut settings = GLOBALS.settings.blocking_write();
                settings.encrypted_private_key = None;
                task::spawn(async move {
                    if let Err(e) = settings.save().await {
                        tracing::error!("{}", e);
                    }
                });
                *self = Signer::Fresh;
                Ok(())
            }
            _ => Err(Error::NoPrivateKey),
        }
    }
}
