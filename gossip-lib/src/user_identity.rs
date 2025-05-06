use crate::bookmarks::BookmarkList;
use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use nostr_types::{
    nip46, ContentEncryptionAlgorithm, DelegationConditions, EncryptedPrivateKey, Event, EventKind,
    ExportableSigner, Filter, Id, Identity, KeySecurity, LockableSigner, Metadata, PreEvent,
    PrivateKey, PublicKey, Rumor, Signature,
};
use parking_lot::RwLock;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use tokio::task;

#[derive(Debug, Clone)]
pub struct UserIdentity {
    pub inner: Arc<RwLock<Identity>>,
}

impl Default for UserIdentity {
    fn default() -> UserIdentity {
        UserIdentity {
            inner: Arc::new(RwLock::new(Identity::default())),
        }
    }
}

impl UserIdentity {
    pub(crate) fn load(&self) -> Result<(), Error> {
        let identity = GLOBALS.db().read_identity()?.unwrap_or(Identity::None);
        *self.inner.write_arc() = identity;
        Ok(())
    }

    pub fn inner_lockable(&self) -> Option<Arc<dyn LockableSigner>> {
        let binding = self.inner.read_arc();
        match *binding {
            Identity::Private(ref ks) => Some(ks.clone()),
            _ => None,
        }
    }

    // Any function that changes UserIdentity should run this to save back changes
    fn on_change(&self) -> Result<(), Error> {
        let binding = self.inner.read_arc();
        GLOBALS.db().write_identity(&binding, None)?;
        Ok(())
    }

    // Any function that changes UserIdentity and changes the key should run this instead
    fn on_keychange(&self) -> Result<(), Error> {
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

        GLOBALS.ui_invalidate_notes(&dms);

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

    pub(crate) fn set_public_key(&self, public_key: PublicKey) -> Result<(), Error> {
        *self.inner.write_arc() = Identity::Public(public_key);
        self.on_keychange()?;
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

    pub(crate) fn set_private_key(&self, pk: PrivateKey, pass: &str) -> Result<(), Error> {
        let log_n = GLOBALS.db().read_setting_log_n();
        let identity = Identity::from_private_key(pk, pass, log_n)?;
        *self.inner.write_arc() = identity;
        self.on_keychange()?;
        Ok(())
    }

    pub(crate) fn set_remote_signer(
        &self,
        bunker_client: nip46::BunkerClient,
    ) -> Result<(), Error> {
        let identity = Identity::Remote(bunker_client);
        *self.inner.write_arc() = identity;
        self.on_keychange()?;
        Ok(())
    }

    pub async fn unlock(&self, pass: &str) -> Result<(), Error> {
        self.inner.write_arc().unlock(pass)?;

        // If older version, re-encrypt with new version at default 2^18 rounds
        if let Some(epk) = self.encrypted_private_key() {
            if epk.version()? < 2 {
                let log_n = GLOBALS.db().read_setting_log_n();
                self.inner.write_arc().upgrade(pass, log_n)?;
                self.on_change()?;
            }
        }

        self.on_unlock().await?;

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
        matches!(*self.inner.read_arc(), Identity::Private(_))
    }

    pub async fn export_private_key_bech32(&self, pass: &str) -> Result<(String, bool), Error> {
        let mut binding = self.inner.write_arc();
        match *binding {
            Identity::Private(ref mut ks) => {
                let log_n = GLOBALS.db().read_setting_log_n();
                Ok(ks.export_private_key_in_bech32(pass, log_n).await?)
            }
            _ => Err(ErrorKind::KeyNotExportable.into()),
        }
    }

    pub async fn export_private_key_hex(&self, pass: &str) -> Result<(String, bool), Error> {
        let mut binding = self.inner.write_arc();
        match *binding {
            Identity::Private(ref mut ks) => {
                let log_n = GLOBALS.db().read_setting_log_n();
                Ok(ks.export_private_key_in_hex(pass, log_n).await?)
            }
            _ => Err(ErrorKind::KeyNotExportable.into()),
        }
    }

    pub async fn unwrap_giftwrap(&self, event: &Event) -> Result<Rumor, Error> {
        Ok(self.inner.read_arc().unwrap_giftwrap(event).await?)
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
