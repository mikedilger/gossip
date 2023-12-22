use std::sync::mpsc::Sender;

use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use nostr_types::{
    ContentEncryptionAlgorithm, EncryptedPrivateKey, Event, EventKind, EventV1, Id, KeySecurity,
    PreEvent, PrivateKey, PublicKey, Rumor, RumorV1,
};
use parking_lot::RwLock;
use tokio::task;

/// The signer which holds the user's identity and signs things on their behalf.
#[derive(Default)]
pub struct Signer {
    public: RwLock<Option<PublicKey>>,
    encrypted: RwLock<Option<EncryptedPrivateKey>>,
    private: RwLock<Option<PrivateKey>>,
}

impl Signer {
    pub(crate) fn init(&self) -> Result<(), Error> {
        if self.public.read().is_none() {
            *self.public.write() = GLOBALS.storage.read_setting_public_key();
        }
        if self.encrypted.read().is_none() {
            let epk = GLOBALS.storage.read_encrypted_private_key()?;
            *self.encrypted.write() = epk;
        }

        Ok(())
    }

    pub(crate) fn save(&self) -> Result<(), Error> {
        GLOBALS
            .storage
            .write_setting_public_key(&self.public.read(), None)?;

        let epk = self.encrypted.read().clone();
        GLOBALS.storage.write_encrypted_private_key(&epk, None)?;

        Ok(())
    }

    pub(crate) fn set_public_key(&self, pk: PublicKey) {
        if self.private.read().is_some() {
            GLOBALS
                .status_queue
                .write()
                .write("Ignored setting of public key (private key supercedes)".to_string());
        } else {
            *self.public.write() = Some(pk);
            let _ = self.save();

            // Reubild the event tag index, since the 'p' tags it need to index just changed.
            task::spawn(async move {
                if let Err(e) = GLOBALS.storage.rebuild_event_tags_index(None) {
                    tracing::error!("{}", e);
                }
            });
        }
    }

    pub(crate) fn clear_public_key(&self) {
        if self.private.read().is_some() {
            GLOBALS
                .status_queue
                .write()
                .write("Ignored clearing of public key (private key supercedes)".to_string());
        } else {
            *self.public.write() = None;
            let _ = self.save();
        }
    }

    /// Set the encrypted private key
    ///
    /// Prefer the overlord's import_priv
    pub fn set_encrypted_private_key(&self, epk: EncryptedPrivateKey) {
        if self.private.read().is_some() && self.encrypted.read().is_some() {
            // ignore, epk supercedes
        } else {
            *self.encrypted.write() = Some(epk);
            let _ = self.save();
        }

        // Reubild the event tag index, since the 'p' tags it need to index just changed.
        task::spawn(async move {
            if let Err(e) = GLOBALS.storage.rebuild_event_tags_index(None) {
                tracing::error!("{}", e);
            }
        });
    }

    pub(crate) fn set_private_key(&self, pk: PrivateKey, pass: &str) -> Result<(), Error> {
        *self.encrypted.write() =
            Some(pk.export_encrypted(pass, GLOBALS.storage.read_setting_log_n())?);
        *self.public.write() = Some(pk.public_key());
        *self.private.write() = Some(pk);
        self.save()?;

        // Reubild the event tag index, since the 'p' tags it need to index just changed.
        task::spawn(async move {
            if let Err(e) = GLOBALS.storage.rebuild_event_tags_index(None) {
                tracing::error!("{}", e);
            }
        });

        Ok(())
    }

    /// Unlock the encrypted private key
    ///
    /// Prefer the overlord's unlock_key
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
                    self.save()?;
                }

                if self.public.read().is_none() {
                    *self.public.write() = Some(private.public_key());
                    self.save()?;
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

                // Update wait for login condition
                GLOBALS
                    .wait_for_login
                    .store(false, std::sync::atomic::Ordering::Relaxed);
                GLOBALS.wait_for_login_notify.notify_one();
            }

            Ok(())
        } else {
            Err((ErrorKind::NoPrivateKey, file!(), line!()).into())
        }
    }

    pub(crate) async fn change_passphrase(&self, old: &str, new: &str) -> Result<(), Error> {
        let maybe_encrypted = self.encrypted.read().to_owned();
        match maybe_encrypted {
            Some(epk) => {
                // Test password
                let pk = epk.decrypt(old)?;
                let epk = pk.export_encrypted(new, GLOBALS.storage.read_setting_log_n())?;
                *self.encrypted.write() = Some(epk);
                self.save()?;
                GLOBALS
                    .status_queue
                    .write()
                    .write("Passphrase changed.".to_owned());
                Ok(())
            }
            _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    pub(crate) fn generate_private_key(&self, pass: &str) -> Result<(), Error> {
        let pk = PrivateKey::generate();
        *self.encrypted.write() =
            Some(pk.export_encrypted(pass, GLOBALS.storage.read_setting_log_n())?);
        *self.public.write() = Some(pk.public_key());
        *self.private.write() = Some(pk);
        self.save()?;

        // and eventually save
        task::spawn(async move {
            // Reubild the event tag index, since the 'p' tags it need to index just changed.
            if let Err(e) = GLOBALS.storage.rebuild_event_tags_index(None) {
                tracing::error!("{}", e);
            }
        });

        Ok(())
    }

    /// Is the private key loaded (possibly still encrypted)?
    pub fn is_loaded(&self) -> bool {
        self.encrypted.read().is_some() || self.private.read().is_some()
    }

    /// Is the private key unlocked and ready for signing?
    pub fn is_ready(&self) -> bool {
        self.private.read().is_some()
    }

    /// Get the public key
    ///
    /// Often you'll want to get this from Settings instead, especially if you need to see
    /// if it exists even before a user logs in.
    pub fn public_key(&self) -> Option<PublicKey> {
        *self.public.read()
    }

    /// Get the encrypted private key
    pub fn encrypted_private_key(&self) -> Option<EncryptedPrivateKey> {
        self.encrypted.read().clone()
    }

    /// How secure is the private key? Dig into `KeySecurity` to understand this better.
    pub fn key_security(&self) -> Option<KeySecurity> {
        self.private.read().as_ref().map(|pk| pk.key_security())
    }

    pub(crate) fn sign_preevent(
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

    /// Export the private key as bech32 (decrypted!)
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
                self.save()?;
                Ok(output)
            }
            _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    /// Export the private key as hex (decrypted!)
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
                self.save()?;
                Ok(output)
            }
            _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    pub(crate) fn delete_identity(&self) {
        *self.private.write() = None;
        *self.encrypted.write() = None;
        *self.public.write() = None;
        let _ = self.save();
    }

    /// Decrypt an event
    pub fn decrypt_message(&self, event: &Event) -> Result<String, Error> {
        match &*self.private.read() {
            Some(private) => Ok(event.decrypted_contents(private)?),
            _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    /// Unwrap a giftwrap event
    pub fn unwrap_giftwrap(&self, event: &Event) -> Result<Rumor, Error> {
        match &*self.private.read() {
            Some(private) => Ok(event.giftwrap_unwrap(private)?),
            _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    /// Unwrap a giftwrap event V1
    pub fn unwrap_giftwrap1(&self, event: &EventV1) -> Result<RumorV1, Error> {
        match &*self.private.read() {
            Some(private) => Ok(event.giftwrap_unwrap(private)?),
            _ => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    /// Encrypt content
    pub fn encrypt(
        &self,
        other: &PublicKey,
        plaintext: &str,
        algo: ContentEncryptionAlgorithm,
    ) -> Result<String, Error> {
        match &*self.private.read() {
            Some(private) => Ok(private.encrypt(other, plaintext, algo)?),
            None => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    /// Decrypt NIP-04 content
    pub fn decrypt_nip04(&self, other: &PublicKey, ciphertext: &str) -> Result<Vec<u8>, Error> {
        match &*self.private.read() {
            Some(private) => Ok(private.decrypt_nip04(other, ciphertext)?),
            None => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }

    /// Decrypt NIP-44 content
    pub fn decrypt_nip44(&self, other: &PublicKey, ciphertext: &str) -> Result<String, Error> {
        match &*self.private.read() {
            Some(private) => Ok(private.decrypt_nip44(other, ciphertext)?),
            None => Err((ErrorKind::NoPrivateKey, file!(), line!()).into()),
        }
    }
}
