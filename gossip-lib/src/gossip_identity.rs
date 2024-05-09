use std::sync::mpsc::Sender;

use nostr_types::{
    ContentEncryptionAlgorithm, DelegationConditions, EncryptedPrivateKey, Event, EventKind,
    EventV1, EventV2, Filter, Id, Identity, KeySecurity, Metadata, PreEvent, PrivateKey, PublicKey,
    Rumor, RumorV1, RumorV2, Signature,
};
use parking_lot::RwLock;
use tokio::task;

use crate::error::Error;
use crate::globals::GLOBALS;

pub struct GossipIdentity {
    pub inner: RwLock<Identity>,
}

impl Default for GossipIdentity {
    fn default() -> GossipIdentity {
        GossipIdentity {
            inner: RwLock::new(Identity::default()),
        }
    }
}

impl GossipIdentity {
    pub(crate) fn load(&self) -> Result<(), Error> {
        let pk = GLOBALS.storage.read_setting_public_key();
        let epk = GLOBALS.storage.read_encrypted_private_key()?;
        match (pk, epk) {
            (Some(pk), Some(epk)) => *self.inner.write() = Identity::from_locked_parts(pk, epk),
            (Some(pk), None) => *self.inner.write() = Identity::Public(pk),
            (None, _) => *self.inner.write() = Identity::None,
        }
        Ok(())
    }

    // Any function that changes GossipIdentity should run this to save back changes
    fn on_change(&self) -> Result<(), Error> {
        let binding = self.inner.read();
        let (pk, epk) = match *binding {
            Identity::None => (None, None),
            Identity::Public(pk) => (Some(pk), None),
            Identity::Signer(ref bs) => (Some(bs.public_key()), bs.encrypted_private_key()),
        };
        GLOBALS.storage.write_setting_public_key(&pk, None)?;
        GLOBALS.storage.write_encrypted_private_key(epk, None)?;
        Ok(())
    }

    // Any function that changes GossipIdentity and changes the key should run this
    // instead
    fn on_keychange(&self) -> Result<(), Error> {
        self.on_change()?;
        if !matches!(*self.inner.read(), Identity::None) {
            // Rebuild the event tag index if the identity changes
            // since the 'p' tags it needs to index just changed.
            task::spawn(async move {
                if let Err(e) = GLOBALS.storage.rebuild_event_tags_index(None) {
                    tracing::error!("{}", e);
                }
            });
        }

        Ok(())
    }

    // Any function that unlocks the private key should run this
    fn on_unlock(&self) -> Result<(), Error> {
        let mut filter = Filter::new();
        filter.kinds = vec![EventKind::EncryptedDirectMessage, EventKind::GiftWrap];

        // Invalidate DMs so they rerender decrypted
        let dms: Vec<Id> = GLOBALS
            .storage
            .find_events_by_filter(&filter, |_| true)?
            .iter()
            .map(|e| e.id)
            .collect();

        GLOBALS.ui_notes_to_invalidate.write().extend(dms);

        // Index any waiting GiftWraps
        GLOBALS.storage.index_unindexed_giftwraps()?;

        // Update wait for login condition
        GLOBALS
            .wait_for_login
            .store(false, std::sync::atomic::Ordering::Relaxed);
        GLOBALS.wait_for_login_notify.notify_one();

        Ok(())
    }

    pub(crate) fn set_public_key(&self, public_key: PublicKey) -> Result<(), Error> {
        *self.inner.write() = Identity::Public(public_key);
        self.on_keychange()?;
        Ok(())
    }

    pub(crate) fn clear_public_key(&self) -> Result<(), Error> {
        *self.inner.write() = Identity::None;
        self.on_keychange()?;
        Ok(())
    }

    pub fn set_encrypted_private_key(
        &self,
        epk: EncryptedPrivateKey,
        pass: &str,
    ) -> Result<(), Error> {
        *self.inner.write() = Identity::from_encrypted_private_key(epk, pass)?;
        self.on_keychange()?;
        Ok(())
    }

    pub(crate) async fn change_passphrase(&self, old: &str, new: &str) -> Result<(), Error> {
        let log_n = GLOBALS.storage.read_setting_log_n();
        self.inner.write().change_passphrase(old, new, log_n)?;
        Ok(())
    }

    pub(crate) fn set_private_key(&self, pk: PrivateKey, pass: &str) -> Result<(), Error> {
        let log_n = GLOBALS.storage.read_setting_log_n();
        let identity = Identity::from_private_key(pk, pass, log_n)?;
        *self.inner.write() = identity;
        self.on_keychange()?;
        Ok(())
    }

    pub fn unlock(&self, pass: &str) -> Result<(), Error> {
        self.inner.write().unlock(pass)?;

        // If older version, re-encrypt with new version at default 2^18 rounds
        if let Some(epk) = self.encrypted_private_key() {
            if epk.version()? < 2 {
                let log_n = GLOBALS.storage.read_setting_log_n();
                self.inner.write().upgrade(pass, log_n)?;
                self.on_change()?;
            }
        }

        self.on_unlock()?;

        Ok(())
    }

    pub(crate) fn generate_private_key(&self, pass: &str) -> Result<(), Error> {
        let log_n = GLOBALS.storage.read_setting_log_n();
        *self.inner.write() = Identity::generate(pass, log_n)?;
        self.on_keychange()?;
        Ok(())
    }

    pub(crate) fn delete_identity(&self) -> Result<(), Error> {
        *self.inner.write() = Identity::None;
        self.on_keychange()?;
        Ok(())
    }

    pub fn has_private_key(&self) -> bool {
        self.inner.read().has_private_key()
    }

    pub fn is_unlocked(&self) -> bool {
        self.inner.read().is_unlocked()
    }

    pub fn public_key(&self) -> Option<PublicKey> {
        self.inner.read().public_key()
    }

    pub fn encrypted_private_key(&self) -> Option<EncryptedPrivateKey> {
        self.inner.read().encrypted_private_key().cloned()
    }

    pub fn key_security(&self) -> Result<KeySecurity, Error> {
        Ok(self.inner.read().key_security()?)
    }

    pub fn sign_event(&self, input: PreEvent) -> Result<Event, Error> {
        Ok(self.inner.read().sign_event(input)?)
    }

    pub fn sign_event_with_pow(
        &self,
        input: PreEvent,
        zero_bits: u8,
        work_sender: Option<Sender<u8>>,
    ) -> Result<Event, Error> {
        Ok(self
            .inner
            .read()
            .sign_event_with_pow(input, zero_bits, work_sender)?)
    }

    pub fn export_private_key_bech32(&self, pass: &str) -> Result<(String, bool), Error> {
        let log_n = GLOBALS.storage.read_setting_log_n();
        Ok(self
            .inner
            .write()
            .export_private_key_in_bech32(pass, log_n)?)
    }

    pub fn export_private_key_hex(&self, pass: &str) -> Result<(String, bool), Error> {
        let log_n = GLOBALS.storage.read_setting_log_n();
        Ok(self.inner.write().export_private_key_in_hex(pass, log_n)?)
    }

    pub fn unwrap_giftwrap(&self, event: &Event) -> Result<Rumor, Error> {
        Ok(self.inner.read().unwrap_giftwrap(event)?)
    }

    /// @deprecated for migrations only
    pub fn unwrap_giftwrap1(&self, event: &EventV1) -> Result<RumorV1, Error> {
        Ok(self.inner.read().unwrap_giftwrap1(event)?)
    }

    /// @deprecated for migrations only
    pub fn unwrap_giftwrap2(&self, event: &EventV2) -> Result<RumorV2, Error> {
        Ok(self.inner.read().unwrap_giftwrap2(event)?)
    }

    pub fn decrypt_event_contents(&self, event: &Event) -> Result<String, Error> {
        Ok(self.inner.read().decrypt_event_contents(event)?)
    }

    pub fn decrypt(&self, other: &PublicKey, ciphertext: &str) -> Result<String, Error> {
        Ok(self.inner.read().decrypt(other, ciphertext)?)
    }

    pub fn nip44_conversation_key(&self, other: &PublicKey) -> Result<[u8; 32], Error> {
        Ok(self.inner.read().nip44_conversation_key(other)?)
    }

    pub fn encrypt(
        &self,
        other: &PublicKey,
        plaintext: &str,
        algo: ContentEncryptionAlgorithm,
    ) -> Result<String, Error> {
        Ok(self.inner.read().encrypt(other, plaintext, algo)?)
    }

    pub fn create_metadata_event(
        &self,
        input: PreEvent,
        metadata: Metadata,
    ) -> Result<Event, Error> {
        Ok(self.inner.read().create_metadata_event(input, metadata)?)
    }

    pub fn create_zap_request_event(
        &self,
        recipient_pubkey: PublicKey,
        zapped_event: Option<Id>,
        millisatoshis: u64,
        relays: Vec<String>,
        content: String,
    ) -> Result<Event, Error> {
        Ok(self.inner.read().create_zap_request_event(
            recipient_pubkey,
            zapped_event,
            millisatoshis,
            relays,
            content,
        )?)
    }

    pub fn generate_delegation_signature(
        &self,
        delegated_pubkey: PublicKey,
        delegation_conditions: &DelegationConditions,
    ) -> Result<Signature, Error> {
        Ok(self
            .inner
            .read()
            .generate_delegation_signature(delegated_pubkey, delegation_conditions)?)
    }

    pub fn giftwrap(&self, input: PreEvent, pubkey: PublicKey) -> Result<Event, Error> {
        Ok(self.inner.read().giftwrap(input, pubkey)?)
    }

    pub fn verify_delegation_signature(
        &self,
        delegated_pubkey: PublicKey,
        delegation_conditions: &DelegationConditions,
        signature: &Signature,
    ) -> Result<(), Error> {
        Ok(self.inner.read().verify_delegation_signature(
            delegated_pubkey,
            delegation_conditions,
            signature,
        )?)
    }
}
