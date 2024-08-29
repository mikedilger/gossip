use crate::bookmarks::BookmarkList;
use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{
    ContentEncryptionAlgorithm, DelegationConditions, EncryptedPrivateKey, Event, EventKind,
    EventV1, EventV2, Filter, Id, Identity, KeySecurity, Metadata, PreEvent, PrivateKey, PublicKey,
    Rumor, RumorV1, RumorV2, Signature,
};
use parking_lot::RwLock;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use tokio::task;

pub struct GossipIdentity {
    pub inner: Arc<RwLock<Identity>>,
}

impl Default for GossipIdentity {
    fn default() -> GossipIdentity {
        GossipIdentity {
            inner: Arc::new(RwLock::new(Identity::default())),
        }
    }
}

impl GossipIdentity {
    pub(crate) fn load(&self) -> Result<(), Error> {
        let pk = GLOBALS.db().read_setting_public_key();
        let epk = GLOBALS.db().read_encrypted_private_key()?;
        match (pk, epk) {
            (Some(pk), Some(epk)) => *self.inner.write_arc() = Identity::from_locked_parts(pk, epk),
            (Some(pk), None) => *self.inner.write_arc() = Identity::Public(pk),
            (None, _) => *self.inner.write_arc() = Identity::None,
        }
        Ok(())
    }

    // Any function that changes GossipIdentity should run this to save back changes
    fn on_change(&self) -> Result<(), Error> {
        let binding = self.inner.read_arc();
        let (pk, epk) = match *binding {
            Identity::None => (None, None),
            Identity::Public(pk) => (Some(pk), None),
            Identity::Signer(ref bs) => (Some(bs.public_key()), bs.encrypted_private_key()),
        };
        GLOBALS.db().write_setting_public_key(&pk, None)?;
        GLOBALS.db().write_encrypted_private_key(epk, None)?;
        Ok(())
    }

    // Any function that changes GossipIdentity and changes the key should run this instead
    async fn on_keychange(&self) -> Result<(), Error> {
        self.on_change()?;
        if !matches!(*self.inner.read_arc(), Identity::None) {
            // Rebuild the event tag index if the identity changes
            // since the 'p' tags it needs to index just changed.
            task::spawn(async move {
                if let Err(e) = GLOBALS.db().rebuild_event_tags_index(None).await {
                    tracing::error!("{}", e);
                }
            });
        }

        Ok(())
    }

    // Any function that unlocks the private key should run this
    async fn on_unlock(&self) -> Result<(), Error> {
        let mut filter = Filter::new();
        filter.kinds = vec![EventKind::EncryptedDirectMessage, EventKind::GiftWrap];

        // Invalidate DMs so they rerender decrypted
        let dms: Vec<Id> = GLOBALS
            .db()
            .find_events_by_filter(&filter, |_| true)?
            .iter()
            .map(|e| e.id)
            .collect();

        GLOBALS.ui_notes_to_invalidate.write().extend(dms);

        // Recompute bookmarks (including the private part)
        if let Some(pk) = self.public_key() {
            if let Some(event) =
                GLOBALS
                    .db()
                    .get_replaceable_event(EventKind::BookmarkList, pk, "")?
            {
                *GLOBALS.bookmarks.write_arc() = BookmarkList::from_event(&event).await?;
                GLOBALS.recompute_current_bookmarks.notify_one();
            }
        }

        // Index any waiting GiftWraps
        GLOBALS.db().index_unindexed_giftwraps().await?;

        // Update wait for login condition
        GLOBALS
            .wait_for_login
            .store(false, std::sync::atomic::Ordering::Relaxed);
        GLOBALS.wait_for_login_notify.notify_one();

        Ok(())
    }

    pub(crate) async fn set_public_key(&self, public_key: PublicKey) -> Result<(), Error> {
        *self.inner.write_arc() = Identity::Public(public_key);
        self.on_keychange().await?;
        Ok(())
    }

    pub(crate) async fn clear_public_key(&self) -> Result<(), Error> {
        *self.inner.write_arc() = Identity::None;
        self.on_keychange().await?;
        Ok(())
    }

    pub async fn set_encrypted_private_key(
        &self,
        epk: EncryptedPrivateKey,
        pass: &str,
    ) -> Result<(), Error> {
        *self.inner.write_arc() = Identity::from_encrypted_private_key(epk, pass).await?;
        self.on_keychange().await?;
        Ok(())
    }

    pub(crate) async fn change_passphrase(&self, old: &str, new: &str) -> Result<(), Error> {
        let log_n = GLOBALS.db().read_setting_log_n();
        self.inner
            .write_arc()
            .change_passphrase(old, new, log_n)
            .await?;
        self.on_keychange().await?;
        Ok(())
    }

    pub(crate) async fn set_private_key(&self, pk: PrivateKey, pass: &str) -> Result<(), Error> {
        let log_n = GLOBALS.db().read_setting_log_n();
        let identity = Identity::from_private_key(pk, pass, log_n).await?;
        *self.inner.write_arc() = identity;
        self.on_keychange().await?;
        Ok(())
    }

    pub async fn unlock(&self, pass: &str) -> Result<(), Error> {
        self.inner.write_arc().unlock(pass).await?;

        // If older version, re-encrypt with new version at default 2^18 rounds
        if let Some(epk) = self.encrypted_private_key() {
            if epk.version()? < 2 {
                let log_n = GLOBALS.db().read_setting_log_n();
                self.inner.write_arc().upgrade(pass, log_n).await?;
                self.on_change()?;
            }
        }

        self.on_unlock().await?;

        Ok(())
    }

    pub(crate) async fn generate_private_key(&self, pass: &str) -> Result<(), Error> {
        let log_n = GLOBALS.db().read_setting_log_n();
        *self.inner.write_arc() = Identity::generate(pass, log_n).await?;
        self.on_keychange().await?;
        Ok(())
    }

    pub(crate) async fn delete_identity(&self) -> Result<(), Error> {
        *self.inner.write_arc() = Identity::None;
        self.on_keychange().await?;
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
        self.inner.read_arc().encrypted_private_key().cloned()
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

    pub async fn export_private_key_bech32(&self, pass: &str) -> Result<(String, bool), Error> {
        let log_n = GLOBALS.db().read_setting_log_n();
        Ok(self
            .inner
            .write_arc()
            .export_private_key_in_bech32(pass, log_n)
            .await?)
    }

    pub async fn export_private_key_hex(&self, pass: &str) -> Result<(String, bool), Error> {
        let log_n = GLOBALS.db().read_setting_log_n();
        Ok(self
            .inner
            .write_arc()
            .export_private_key_in_hex(pass, log_n)
            .await?)
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

    pub async fn create_metadata_event(
        &self,
        input: PreEvent,
        metadata: Metadata,
    ) -> Result<Event, Error> {
        Ok(self
            .inner
            .read_arc()
            .create_metadata_event(input, metadata)
            .await?)
    }

    pub async fn create_zap_request_event(
        &self,
        recipient_pubkey: PublicKey,
        zapped_event: Option<Id>,
        millisatoshis: u64,
        relays: Vec<String>,
        content: String,
    ) -> Result<Event, Error> {
        Ok(self
            .inner
            .read_arc()
            .create_zap_request_event(
                recipient_pubkey,
                zapped_event,
                millisatoshis,
                relays,
                content,
            )
            .await?)
    }

    pub async fn generate_delegation_signature(
        &self,
        delegated_pubkey: PublicKey,
        delegation_conditions: &DelegationConditions,
    ) -> Result<Signature, Error> {
        Ok(self
            .inner
            .read_arc()
            .generate_delegation_signature(delegated_pubkey, delegation_conditions)
            .await?)
    }

    pub async fn giftwrap(&self, input: PreEvent, pubkey: PublicKey) -> Result<Event, Error> {
        Ok(self.inner.read_arc().giftwrap(input, pubkey).await?)
    }

    pub fn verify_delegation_signature(
        &self,
        delegated_pubkey: PublicKey,
        delegation_conditions: &DelegationConditions,
        signature: &Signature,
    ) -> Result<(), Error> {
        Ok(self.inner.read_arc().verify_delegation_signature(
            delegated_pubkey,
            delegation_conditions,
            signature,
        )?)
    }
}
