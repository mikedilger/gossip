use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use nostr_types::{
    ContentEncryptionAlgorithm, EncryptedPrivateKey, Event, EventV1, EventV2, Identity,
    KeySecurity, PreEvent, PublicKey, Rumor, RumorV1, RumorV2,
};
use parking_lot::RwLock;
use std::sync::mpsc::Sender;
use std::sync::Arc;

pub struct ClientIdentity {
    pub inner: Arc<RwLock<Identity>>,
}

impl Default for ClientIdentity {
    fn default() -> ClientIdentity {
        ClientIdentity {
            inner: Arc::new(RwLock::new(Identity::default())),
        }
    }
}

impl ClientIdentity {
    pub(crate) fn load(&self) -> Result<(), Error> {
        let pk = GLOBALS.db().read_setting_client_public_key();
        let epk = GLOBALS.db().read_client_encrypted_private_key()?;
        match (pk, epk) {
            (Some(pk), Some(epk)) => *self.inner.write_arc() = Identity::from_locked_parts(pk, epk),
            (Some(pk), None) => *self.inner.write_arc() = Identity::Public(pk),
            (None, _) => *self.inner.write_arc() = Identity::None,
        }
        Ok(())
    }

    // Any function that changes ClientIdentity should run this to save back changes
    fn on_change(&self) -> Result<(), Error> {
        let binding = self.inner.read_arc();
        let (pk, epk) = match *binding {
            Identity::None => (None, None),
            Identity::Public(pk) => (Some(pk), None),
            Identity::LockableSigner(ref bs) => (Some(bs.public_key()), bs.encrypted_private_key()),
            Identity::FullSigner(ref bs) => (Some(bs.public_key()), bs.encrypted_private_key()),
        };
        GLOBALS.db().write_setting_client_public_key(&pk, None)?;
        GLOBALS.db().write_client_encrypted_private_key(epk, None)?;
        Ok(())
    }

    // Any function that changes ClientIdentity and changes the key should run this instead
    fn on_keychange(&self) -> Result<(), Error> {
        self.on_change()?;
        Ok(())
    }

    // Any function that unlocks the private key should run this
    fn on_unlock(&self) -> Result<(), Error> {
        Ok(())
    }

    pub(crate) fn clear_public_key(&self) -> Result<(), Error> {
        *self.inner.write_arc() = Identity::None;
        self.on_keychange()?;
        Ok(())
    }

    pub fn set_encrypted_private_key(
        &self,
        epk: EncryptedPrivateKey,
        pass: &str,
    ) -> Result<(), Error> {
        *self.inner.write_arc() = Identity::from_encrypted_private_key(epk, pass)?;
        self.on_keychange()?;
        Ok(())
    }

    pub(crate) async fn change_passphrase(&self, old: &str, new: &str) -> Result<(), Error> {
        let log_n = GLOBALS.db().read_setting_log_n();
        self.inner.write_arc().change_passphrase(old, new, log_n)?;
        self.on_keychange()?;
        Ok(())
    }

    pub fn unlock(&self, pass: &str) -> Result<(), Error> {
        self.inner.write_arc().unlock(pass)?;

        // If older version, re-encrypt with new version at default 2^18 rounds
        if let Some(epk) = self.encrypted_private_key() {
            if epk.version()? < 2 {
                let log_n = GLOBALS.db().read_setting_log_n();
                self.inner.write_arc().upgrade(pass, log_n)?;
                self.on_change()?;
            }
        }

        self.on_unlock()?;

        Ok(())
    }

    pub(crate) fn generate_private_key(&self, pass: &str) -> Result<(), Error> {
        let log_n = GLOBALS.db().read_setting_log_n();
        *self.inner.write_arc() = Identity::generate(pass, log_n)?;
        self.on_keychange()?;
        Ok(())
    }

    pub(crate) fn delete_identity(&self) -> Result<(), Error> {
        *self.inner.write_arc() = Identity::None;
        self.on_keychange()?;
        Ok(())
    }

    pub fn has_private_key(&self) -> bool {
        self.inner.read_arc().has_private_key()
    }

    pub fn is_unlocked(&self) -> bool {
        self.inner.read_arc().is_unlocked()
    }

    pub fn public_key(&self) -> Option<PublicKey> {
        self.inner.read_arc().public_key()
    }

    pub fn encrypted_private_key(&self) -> Option<EncryptedPrivateKey> {
        self.inner.read_arc().encrypted_private_key()
    }

    pub fn key_security(&self) -> Result<KeySecurity, Error> {
        Ok(self.inner.read_arc().key_security()?)
    }

    pub async fn sign_event(&self, input: PreEvent) -> Result<Event, Error> {
        Ok(self.inner.read_arc().sign_event(input).await?)
    }

    pub async fn sign_event_with_pow(
        &self,
        input: PreEvent,
        zero_bits: u8,
        work_sender: Option<Sender<u8>>,
    ) -> Result<Event, Error> {
        Ok(self
            .inner
            .read_arc()
            .sign_event_with_pow(input, zero_bits, work_sender)
            .await?)
    }

    pub fn key_is_exportable(&self) -> bool {
        matches!(*self.inner.read_arc(), Identity::FullSigner(_))
    }

    pub async fn export_private_key_bech32(&self, pass: &str) -> Result<(String, bool), Error> {
        let mut binding = self.inner.write_arc();
        match *binding {
            Identity::FullSigner(ref mut bs) => {
                let log_n = GLOBALS.db().read_setting_log_n();
                Ok(bs.export_private_key_in_bech32(pass, log_n).await?)
            }
            _ => Err(ErrorKind::KeyNotExportable.into()),
        }
    }

    pub async fn export_private_key_hex(&self, pass: &str) -> Result<(String, bool), Error> {
        let mut binding = self.inner.write_arc();
        match *binding {
            Identity::FullSigner(ref mut bs) => {
                let log_n = GLOBALS.db().read_setting_log_n();
                Ok(bs.export_private_key_in_hex(pass, log_n).await?)
            }
            _ => Err(ErrorKind::KeyNotExportable.into()),
        }
    }

    pub async fn unwrap_giftwrap(&self, event: &Event) -> Result<Rumor, Error> {
        Ok(self.inner.read_arc().unwrap_giftwrap(event).await?)
    }

    /// @deprecated for migrations only
    pub async fn unwrap_giftwrap1(&self, event: &EventV1) -> Result<RumorV1, Error> {
        Ok(self.inner.read_arc().unwrap_giftwrap1(event).await?)
    }

    /// @deprecated for migrations only
    pub async fn unwrap_giftwrap2(&self, event: &EventV2) -> Result<RumorV2, Error> {
        Ok(self.inner.read_arc().unwrap_giftwrap2(event).await?)
    }

    pub async fn decrypt_event_contents(&self, event: &Event) -> Result<String, Error> {
        Ok(self.inner.read_arc().decrypt_event_contents(event).await?)
    }

    pub async fn decrypt(&self, other: &PublicKey, ciphertext: &str) -> Result<String, Error> {
        Ok(self.inner.read_arc().decrypt(other, ciphertext).await?)
    }

    pub async fn nip44_conversation_key(&self, other: &PublicKey) -> Result<[u8; 32], Error> {
        Ok(self.inner.read_arc().nip44_conversation_key(other).await?)
    }

    pub async fn encrypt(
        &self,
        other: &PublicKey,
        plaintext: &str,
        algo: ContentEncryptionAlgorithm,
    ) -> Result<String, Error> {
        Ok(self
            .inner
            .read_arc()
            .encrypt(other, plaintext, algo)
            .await?)
    }
}
