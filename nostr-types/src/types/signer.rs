use crate::{
    ContentEncryptionAlgorithm, DelegationConditions, EncryptedPrivateKey, Error, Event, EventKind,
    Id, KeySecurity, KeySigner, Metadata, ParsedTag, PreEvent, PrivateKey, PublicKey, Rumor,
    Signature, Tag, Unixtime,
};
use async_trait::async_trait;
use rand::Rng;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;

/// Signer operations
#[async_trait]
pub trait Signer: fmt::Debug + Send + Sync {
    /// What is the signer's public key?
    fn public_key(&self) -> PublicKey;

    /// What is the signer's encrypted private key?
    fn encrypted_private_key(&self) -> Option<EncryptedPrivateKey>;

    /// Sign an event
    async fn sign_event(&self, input: PreEvent) -> Result<Event, Error>;

    /*
    /// Sign an event
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
     */

    /// Encrypt
    async fn encrypt(
        &self,
        other: &PublicKey,
        plaintext: &str,
        algo: ContentEncryptionAlgorithm,
    ) -> Result<String, Error>;

    /// Decrypt NIP-04 or NIP-44
    async fn decrypt(&self, other: &PublicKey, ciphertext: &str) -> Result<String, Error>;

    /// Get the security level of the private key
    fn key_security(&self) -> Result<KeySecurity, Error>;

    /// Giftwrap an event
    async fn giftwrap(&self, input: PreEvent, pubkey: PublicKey) -> Result<Event, Error> {
        let sender_pubkey = input.pubkey;

        // Verify the pubkey matches
        if sender_pubkey != self.public_key() {
            return Err(Error::InvalidPrivateKey);
        }

        let seal_backdate = Unixtime(
            input.created_at.0
                - rand::rng().sample(rand::distr::Uniform::new(30, 60 * 60 * 24 * 2).unwrap()),
        );
        let giftwrap_backdate = Unixtime(
            input.created_at.0
                - rand::rng().sample(rand::distr::Uniform::new(30, 60 * 60 * 24 * 2).unwrap()),
        );

        let seal = {
            let rumor = Rumor::new(input)?;
            let rumor_json = serde_json::to_string(&rumor)?;
            let encrypted_rumor_json = self
                .encrypt(&pubkey, &rumor_json, ContentEncryptionAlgorithm::Nip44v2)
                .await?;

            let pre_seal = PreEvent {
                pubkey: sender_pubkey,
                created_at: seal_backdate,
                kind: EventKind::Seal,
                content: encrypted_rumor_json,
                tags: vec![],
            };

            self.sign_event(pre_seal).await?
        };

        // Generate a random keypair for the gift wrap
        let random_signer = {
            let random_private_key = PrivateKey::generate();
            KeySigner::from_private_key(random_private_key, "", 1)
        }?;

        let seal_json = serde_json::to_string(&seal)?;
        let encrypted_seal_json = random_signer
            .encrypt(&pubkey, &seal_json, ContentEncryptionAlgorithm::Nip44v2)
            .await?;

        let pre_giftwrap = PreEvent {
            pubkey: random_signer.public_key(),
            created_at: giftwrap_backdate,
            kind: EventKind::GiftWrap,
            content: encrypted_seal_json,
            tags: vec![ParsedTag::Pubkey {
                pubkey,
                recommended_relay_url: None,
                petname: None,
            }
            .into_tag()],
        };

        random_signer.sign_event(pre_giftwrap).await
    }

    /// Create an event that sets Metadata
    async fn create_metadata_event(
        &self,
        mut input: PreEvent,
        metadata: Metadata,
    ) -> Result<Event, Error> {
        input.kind = EventKind::Metadata;
        input.content = serde_json::to_string(&metadata)?;
        self.sign_event(input).await
    }

    /// Create a ZapRequest event
    /// These events are not published to nostr, they are sent to a lnurl.
    async fn create_zap_request_event(
        &self,
        recipient_pubkey: PublicKey,
        zapped_event: Option<Id>,
        millisatoshis: u64,
        relays: Vec<String>,
        content: String,
    ) -> Result<Event, Error> {
        let mut relays_tag = Tag::new(&["relays"]);
        relays_tag.push_values(relays);

        let mut pre_event = PreEvent {
            pubkey: self.public_key(),
            created_at: Unixtime::now(),
            kind: EventKind::ZapRequest,
            tags: vec![
                ParsedTag::Pubkey {
                    pubkey: recipient_pubkey,
                    recommended_relay_url: None,
                    petname: None,
                }
                .into_tag(),
                relays_tag,
                Tag::new(&["amount", &format!("{millisatoshis}")]),
            ],
            content,
        };

        if let Some(ze) = zapped_event {
            pre_event.tags.push(
                ParsedTag::Event {
                    id: ze,
                    recommended_relay_url: None,
                    marker: None,
                    author_pubkey: None,
                }
                .into_tag(),
            );
        }

        self.sign_event(pre_event).await
    }

    /// Decrypt the contents of an event
    async fn decrypt_event_contents(&self, event: &Event) -> Result<String, Error> {
        if !event.kind.contents_are_encrypted() {
            return Err(Error::WrongEventKind);
        }

        let pubkey = if event.pubkey == self.public_key() {
            // If you are the author, get the other pubkey from the tags
            event
                .people()
                .iter()
                .filter_map(|(pk, _, _)| if *pk != event.pubkey { Some(*pk) } else { None })
                .nth(0)
                .unwrap_or(event.pubkey) // in case you sent it to yourself.
        } else {
            event.pubkey
        };

        self.decrypt(&pubkey, &event.content).await
    }

    /// If a gift wrap event, unwrap and return the inner Rumor
    async fn unwrap_giftwrap(&self, event: &Event) -> Result<Rumor, Error> {
        if event.kind != EventKind::GiftWrap {
            return Err(Error::WrongEventKind);
        }

        // Verify you are tagged
        let mut tagged = false;
        for t in event.tags.iter() {
            if let Ok(ParsedTag::Pubkey { pubkey, .. }) = t.parse() {
                if pubkey == self.public_key() {
                    tagged = true;
                }
            }
        }
        if !tagged {
            return Err(Error::InvalidRecipient);
        }

        // Decrypt the content
        let content = self.decrypt(&event.pubkey, &event.content).await?;

        // Translate into a seal Event
        let seal: Event = serde_json::from_str(&content)?;

        // Verify it is a Seal
        if seal.kind != EventKind::Seal {
            return Err(Error::WrongEventKind);
        }

        // Veirfy the signature of the seal
        seal.verify(None)?;

        // Note the author
        let author = seal.pubkey;

        // Decrypt the content
        let content = self.decrypt(&seal.pubkey, &seal.content).await?;

        // Translate into a Rumor
        let rumor: Rumor = serde_json::from_str(&content)?;

        // Compare the author
        if rumor.pubkey != author {
            return Err(Error::InvalidPublicKey);
        }

        // Return the Rumor
        Ok(rumor)
    }
}

/// Extended Signer operations
///
/// These enable NIP-26 delegation, signing an event with PoW,
/// signing of anything (not just events) and access to the NIP44 conversation key.
/// none of which is available using NIP-07 (browser) or NIP-46 (bunker) signers.
#[async_trait]
pub trait SignerExt: Signer {
    /// Sign a 32-bit hash asynchronously
    async fn sign_id(&self, id: Id) -> Result<Signature, Error>;

    /// Sign a message asynchronously (this hashes with SHA-256 first internally)
    async fn sign(&self, message: &[u8]) -> Result<Signature, Error>;

    /// Get NIP-44 conversation key
    async fn nip44_conversation_key(&self, other: &PublicKey) -> Result<[u8; 32], Error>;

    /// Generate delegation signature
    async fn generate_delegation_signature(
        &self,
        delegated_pubkey: PublicKey,
        delegation_conditions: &DelegationConditions,
    ) -> Result<Signature, Error> {
        let input = format!(
            "nostr:delegation:{}:{}",
            delegated_pubkey.as_hex_string(),
            delegation_conditions.as_string()
        );

        self.sign(input.as_bytes()).await
    }

    /// Verify delegation signature
    fn verify_delegation_signature(
        &self,
        delegated_pubkey: PublicKey,
        delegation_conditions: &DelegationConditions,
        signature: &Signature,
    ) -> Result<(), Error> {
        let input = format!(
            "nostr:delegation:{}:{}",
            delegated_pubkey.as_hex_string(),
            delegation_conditions.as_string()
        );

        self.public_key().verify(input.as_bytes(), signature)
    }

    /// Sign an event with Proof-of-Work
    async fn sign_event_with_pow(
        &self,
        mut input: PreEvent,
        zero_bits: u8,
        work_sender: Option<Sender<u8>>,
    ) -> Result<Event, Error> {
        let target = format!("{zero_bits}");

        // Verify the pubkey matches
        if input.pubkey != self.public_key() {
            return Err(Error::InvalidPrivateKey);
        }

        // Strip any pre-existing nonce tags
        input.tags.retain(|t| t.tagname() != "nonce");

        // Add nonce tag to the end
        input.tags.push(Tag::new(&["nonce", "0", &target]));
        let index = input.tags.len() - 1;

        let cores = num_cpus::get();

        let quitting = Arc::new(AtomicBool::new(false));
        let nonce = Arc::new(AtomicU64::new(0)); // will store the nonce that works
        let best_work = Arc::new(AtomicU8::new(0));

        let mut join_handles: Vec<JoinHandle<_>> = Vec::with_capacity(cores);

        for core in 0..cores {
            let mut attempt: u64 = core as u64 * (u64::MAX / cores as u64);
            let mut input = input.clone();
            let quitting = quitting.clone();
            let nonce = nonce.clone();
            let best_work = best_work.clone();
            let work_sender = work_sender.clone();
            let join_handle = thread::spawn(move || {
                loop {
                    // Lower the thread priority so other threads aren't starved
                    let _ = thread_priority::set_current_thread_priority(
                        thread_priority::ThreadPriority::Min,
                    );

                    if quitting.load(Ordering::Relaxed) {
                        break;
                    }

                    input.tags[index].set_index(1, format!("{attempt}"));

                    let Id(id) = input.hash().unwrap();

                    let leading_zeroes = crate::get_leading_zero_bits(&id);
                    if leading_zeroes >= zero_bits {
                        nonce.store(attempt, Ordering::Relaxed);
                        quitting.store(true, Ordering::Relaxed);
                        if let Some(sender) = work_sender.clone() {
                            sender.send(leading_zeroes).unwrap();
                        }
                        break;
                    } else if leading_zeroes > best_work.load(Ordering::Relaxed) {
                        best_work.store(leading_zeroes, Ordering::Relaxed);
                        if let Some(sender) = work_sender.clone() {
                            sender.send(leading_zeroes).unwrap();
                        }
                    }

                    attempt += 1;

                    // We don't update created_at, which is a bit tricky to synchronize.
                }
            });
            join_handles.push(join_handle);
        }

        for joinhandle in join_handles {
            let _ = joinhandle.join();
        }

        // We found the nonce. Do it for reals
        input.tags[index].set_index(1, format!("{}", nonce.load(Ordering::Relaxed)));
        let id = input.hash().unwrap();

        // Signature
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
}

/// Any `Signer` that can be locked and unlocked with a passphrase
pub trait LockableSigner: Signer {
    /// Is the signer locked?
    fn is_locked(&self) -> bool;

    /// Try to unlock access to the private key
    fn unlock(&self, password: &str) -> Result<(), Error>;

    /// Lock access to the private key
    fn lock(&self);

    /// Change the passphrase used for locking access to the private key
    fn change_passphrase(&self, old: &str, new: &str, log_n: u8) -> Result<(), Error>;

    /// Upgrade the encrypted private key to the latest format
    fn upgrade(&self, pass: &str, log_n: u8) -> Result<(), Error>;
}

/// Any `Signer` that allows the secret to be exported (with interior mutability
/// or no mutability necessary)
#[async_trait]
pub trait ExportableSigner: Signer {
    /// Export the private key in hex.
    ///
    /// This returns a boolean indicating if the key security was downgraded. If it was,
    /// the caller should save the new self.encrypted_private_key()
    ///
    /// We need the password and log_n parameters to possibly rebuild
    /// the EncryptedPrivateKey when downgrading key security
    async fn export_private_key_in_hex(
        &self,
        pass: &str,
        log_n: u8,
    ) -> Result<(String, bool), Error>;

    /// Export the private key in bech32.
    ///
    /// This returns a boolean indicating if the key security was downgraded. If it was,
    /// the caller should save the new self.encrypted_private_key()
    ///
    /// We need the password and log_n parameters to possibly rebuild
    /// the EncryptedPrivateKey when downgrading key security
    async fn export_private_key_in_bech32(
        &self,
        pass: &str,
        log_n: u8,
    ) -> Result<(String, bool), Error>;
}

/// Any `Signer` that allows the secret to be exported, but requires Self to be mutable
/// to do so.
#[async_trait]
pub trait MutExportableSigner: Signer {
    /// Export the private key in hex.
    ///
    /// This returns a boolean indicating if the key security was downgraded. If it was,
    /// the caller should save the new self.encrypted_private_key()
    ///
    /// We need the password and log_n parameters to possibly rebuild
    /// the EncryptedPrivateKey when downgrading key security
    async fn export_private_key_in_hex(
        &mut self,
        pass: &str,
        log_n: u8,
    ) -> Result<(String, bool), Error>;

    /// Export the private key in bech32.
    ///
    /// This returns a boolean indicating if the key security was downgraded. If it was,
    /// the caller should save the new self.encrypted_private_key()
    ///
    /// We need the password and log_n parameters to possibly rebuild
    /// the EncryptedPrivateKey when downgrading key security
    async fn export_private_key_in_bech32(
        &mut self,
        pass: &str,
        log_n: u8,
    ) -> Result<(String, bool), Error>;
}
