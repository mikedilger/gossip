use std::sync::mpsc::Sender;

use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use nostr_types::{
    EncryptedPrivateKey, Event, EventKind, Id, KeySecurity, PreEvent, PrivateKey, PublicKey, Rumor,
};
use parking_lot::RwLock;
use tokio::task;

#[derive(Default)]
pub struct Signer {
    public: RwLock<Option<PublicKey>>,
    encrypted: RwLock<Option<EncryptedPrivateKey>>,
    private: RwLock<Option<PrivateKey>>,
}

impl Signer {
    pub fn load_from_settings(&self) -> Result<(), Error> {
        if self.public.read().is_none() {
            *self.public.write() = GLOBALS.storage.read_setting_public_key();
        }
        if self.encrypted.read().is_none() {
            let epk = GLOBALS.storage.read_encrypted_private_key()?;
            *self.encrypted.write() = epk;
        }

        Ok(())
    }

    pub async fn save(&self) -> Result<(), Error> {
        GLOBALS
            .storage
            .write_setting_public_key(&self.public.read(), None)?;

        let epk = self.encrypted.read().clone();
        GLOBALS.storage.write_encrypted_private_key(&epk, None)?;

        Ok(())
    }

    pub fn set_public_key(&self, pk: PublicKey) {
        if self.private.read().is_some() {
            GLOBALS
                .status_queue
                .write()
                .write("Ignored setting of public key (private key supercedes)".to_string());
        } else {
            *self.public.write() = Some(pk);
        }
    }

    pub fn clear_public_key(&self) {
        if self.private.read().is_some() {
            GLOBALS
                .status_queue
                .write()
                .write("Ignored clearing of public key (private key supercedes)".to_string());
        } else {
            *self.public.write() = None;
        }
    }

    pub fn set_encrypted_private_key(&self, epk: EncryptedPrivateKey) {
        if self.private.read().is_some() && self.encrypted.read().is_some() {
            // ignore, epk supercedes
        } else {
            *self.encrypted.write() = Some(epk);
        }
    }

    pub fn set_private_key(&self, pk: PrivateKey, pass: &str) -> Result<(), Error> {
        *self.encrypted.write() =
            Some(pk.export_encrypted(pass, GLOBALS.storage.read_setting_log_n())?);
        *self.public.write() = Some(pk.public_key());
        *self.private.write() = Some(pk);
        Ok(())
    }

    pub fn unlock_encrypted_private_key(&self, pass: &str) -> Result<(), Error> {
        if self.private.read().is_some() {
            // ignore, already unlocked
            Ok(())
        } else if let Some(epk) = &*self.encrypted.read() {
            *self.private.write() = Some(epk.decrypt(pass)?);

            if let Some(private) = &*self.private.read() {
                // it will be

                // If older version, re-encrypt with new version at default 2^18 rounds
                if epk.version()? < 2 {
                    *self.encrypted.write() =
                        Some(private.export_encrypted(pass, GLOBALS.storage.read_setting_log_n())?);
                    // and eventually save
                    task::spawn(async move {
                        if let Err(e) = GLOBALS.signer.save().await {
                            tracing::error!("{}", e);
                        }
                    });
                }

                if self.public.read().is_none() {
                    *self.public.write() = Some(private.public_key());
                }

                // Invalidate DMs so they rerender decrypted
                let dms: Vec<Id> = GLOBALS
                    .storage
                    .find_events(
                        &[EventKind::EncryptedDirectMessage, EventKind::GiftWrap],
                        &[],
                        None,
                        |_| true,
                        false,
                    )?
                    .iter()
                    .map(|e| e.id)
                    .collect();

                GLOBALS.ui_notes_to_invalidate.write().extend(dms);

                // Index any GiftWraps that weren't indexed due to not having a
                // private key ready
                GLOBALS.storage.index_unindexed_giftwraps()?;
            }

            Ok(())
        } else {
            Err((ErrorKind::NoPrivateKey, file!(), line!()).into())
        }
    }

    pub fn change_passphrase(&self, old: &str, new: &str) -> Result<(), Error> {
        let maybe_encrypted = self.encrypted.read().to_owned();
        match maybe_encrypted {
            Some(epk) => {
                // Test password
                let pk = epk.decrypt(old)?;
                let epk = pk.export_encrypted(new, GLOBALS.storage.read_setting_log_n())?;
                *self.encrypted.write() = Some(epk);
                task::spawn(async move {
                    if let Err(e) = GLOBALS.signer.save().await {
                        tracing::error!("{}", e);
                    }
                    GLOBALS
                        .status_queue
                        .write()
                        .write("Passphrase changed.".to_owned())
                });
                Ok(())
            }
            _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    pub fn generate_private_key(&self, pass: &str) -> Result<(), Error> {
        let pk = PrivateKey::generate();
        *self.encrypted.write() =
            Some(pk.export_encrypted(pass, GLOBALS.storage.read_setting_log_n())?);
        *self.public.write() = Some(pk.public_key());
        *self.private.write() = Some(pk);

        // and eventually save
        task::spawn(async move {
            if let Err(e) = GLOBALS.signer.save().await {
                tracing::error!("{}", e);
            }
        });

        Ok(())
    }

    pub fn is_loaded(&self) -> bool {
        self.encrypted.read().is_some() || self.private.read().is_some()
    }

    pub fn is_ready(&self) -> bool {
        self.private.read().is_some()
    }

    pub fn public_key(&self) -> Option<PublicKey> {
        *self.public.read()
    }

    pub fn encrypted_private_key(&self) -> Option<EncryptedPrivateKey> {
        self.encrypted.read().clone()
    }

    pub fn key_security(&self) -> Option<KeySecurity> {
        self.private.read().as_ref().map(|pk| pk.key_security())
    }

    pub fn sign_preevent(
        &self,
        preevent: PreEvent,
        pow: Option<u8>,
        work_sender: Option<Sender<u8>>,
    ) -> Result<Event, Error> {
        match &*self.private.read() {
            Some(pk) => match pow {
                Some(pow) => Ok(Event::new_with_pow(preevent, pk, pow, work_sender)?),
                None => Ok(Event::new(preevent, pk)?),
            },
            _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    pub fn new_nip04(
        &self,
        recipient_public_key: PublicKey,
        message: &str,
    ) -> Result<PreEvent, Error> {
        match &*self.private.read() {
            Some(pk) => Ok(PreEvent::new_nip04(pk, recipient_public_key, message)?),
            _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    pub fn export_private_key_bech32(&self, pass: &str) -> Result<String, Error> {
        let maybe_encrypted = self.encrypted.read().to_owned();
        match maybe_encrypted {
            Some(epk) => {
                // Test password
                let mut pk = epk.decrypt(pass)?;

                let output = pk.as_bech32_string();

                // We have to regenerate encrypted private key because it may have fallen from
                // medium to weak security. And then we need to save that
                let epk = pk.export_encrypted(pass, GLOBALS.storage.read_setting_log_n())?;
                *self.encrypted.write() = Some(epk);
                *self.private.write() = Some(pk);
                task::spawn(async move {
                    if let Err(e) = GLOBALS.signer.save().await {
                        tracing::error!("{}", e);
                    }
                });
                Ok(output)
            }
            _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    pub fn export_private_key_hex(&self, pass: &str) -> Result<String, Error> {
        let maybe_encrypted = self.encrypted.read().to_owned();
        match maybe_encrypted {
            Some(epk) => {
                // Test password
                let mut pk = epk.decrypt(pass)?;

                let output = pk.as_hex_string();

                // We have to regenerate encrypted private key because it may have fallen from
                // medium to weak security. And then we need to save that
                let epk = pk.export_encrypted(pass, GLOBALS.storage.read_setting_log_n())?;
                *self.encrypted.write() = Some(epk);
                *self.private.write() = Some(pk);
                task::spawn(async move {
                    if let Err(e) = GLOBALS.signer.save().await {
                        tracing::error!("{}", e);
                    }
                });
                Ok(output)
            }
            _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    pub fn delete_identity(&self) {
        *self.private.write() = None;
        *self.encrypted.write() = None;
        *self.public.write() = None;

        task::spawn(async move {
            if let Err(e) = GLOBALS.signer.save().await {
                tracing::error!("{}", e);
            }
        });
    }

    pub fn decrypt_message(&self, event: &Event) -> Result<String, Error> {
        match &*self.private.read() {
            Some(private) => Ok(event.decrypted_contents(private)?),
            _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    pub fn unwrap_giftwrap(&self, event: &Event) -> Result<Rumor, Error> {
        match &*self.private.read() {
            Some(private) => Ok(event.giftwrap_unwrap(private, false)?),
            _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    pub fn nip44_decrypt(
        &self,
        other: &PublicKey,
        ciphertext: &str,
        padded: bool,
    ) -> Result<Vec<u8>, Error> {
        match &*self.private.read() {
            Some(private) => Ok(private.nip44_decrypt(other, ciphertext, padded)?),
            _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }
}
