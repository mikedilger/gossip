use crate::{
    client, ContentEncryptionAlgorithm, EncryptedPrivateKey, Error, Event, EventKind, Filter,
    KeySecurity, KeySigner, LockableSigner, PreEvent, PublicKey, RelayUrl, Signer, SubscriptionId,
};
use async_trait::async_trait;
use std::sync::{Arc, RwLock};
use std::time::Duration;

mod request;
pub use request::Nip46Request;

mod response;
pub use response::Nip46Response;

mod params;
pub use params::Nip46ConnectionParameters;

mod prebunk;
pub use prebunk::PreBunkerClient;

/// This is a NIP-46 Bunker client
#[derive(Debug)]
pub struct BunkerClient {
    /// The pubkey of the bunker
    pub remote_signer_pubkey: PublicKey,

    /// The relay the bunker is listening at
    pub relay_url: RelayUrl,

    /// Our local identity
    pub local_signer: Arc<KeySigner>,

    /// User Public Key
    pub public_key: PublicKey,

    /// Timeout
    pub timeout: Duration,

    /// Client
    pub client: client::Client,

    /// Sub id
    pub sub_id: RwLock<Option<SubscriptionId>>,
}

impl BunkerClient {
    /// Create a new BunkerClient from stored data. This will be in the locked state.
    pub async fn from_stored_data(
        remote_signer_pubkey: PublicKey,
        relay_url: RelayUrl,
        keysigner: KeySigner,
        public_key: PublicKey,
        timeout: Duration,
    ) -> BunkerClient {
        let client = client::Client::new(relay_url.as_str());
        BunkerClient {
            remote_signer_pubkey,
            relay_url,
            local_signer: Arc::new(keysigner),
            public_key,
            timeout,
            client,
            sub_id: RwLock::new(None),
        }
    }

    /// Is the signer locked?
    pub fn is_locked(&self) -> bool {
        self.local_signer.is_locked()
    }

    /// Unlock (if locked)
    pub fn unlock(&self, password: &str) -> Result<(), Error> {
        self.local_signer.unlock(password)
    }

    /// Lock
    pub fn lock(&self) {
        self.local_signer.lock()
    }

    /// Change passphrase
    pub fn change_passphrase(&self, old: &str, new: &str, log_n: u8) -> Result<(), Error> {
        self.local_signer.change_passphrase(old, new, log_n)
    }

    /// Send a `Nip46Request` and wait for a `Nip46Response` (up to our timeout)
    pub async fn call(&self, request: Nip46Request) -> Result<Nip46Response, Error> {
        // Maybe subscribe
        let missing_sub_id = self.sub_id.read().unwrap().is_none();
        if missing_sub_id {
            let mut filter = Filter::new();
            filter.add_author(self.remote_signer_pubkey);
            filter.add_event_kind(EventKind::NostrConnect);
            filter.add_tag_value('p', self.local_signer.public_key().as_hex_string());
            let sub_id = self.client.subscribe(filter.clone(), self.timeout).await?;
            *self.sub_id.write().unwrap() = Some(sub_id);
        }
        let sub_id = self.sub_id.read().unwrap().clone().unwrap();

        // Post event to server and wait for OK
        let event = request
            .to_event(self.remote_signer_pubkey, self.local_signer.clone())
            .await?;
        let event_id = event.id;
        self.client.post_event(event, self.timeout).await?;
        let (ok, msg) = self.client.wait_for_ok(event_id, self.timeout).await?;
        if !ok {
            return Err(Error::Nip46FailedToPost(msg));
        }

        // Wait for a response
        let event = self
            .client
            .wait_for_subscribed_event(sub_id.clone(), self.timeout)
            .await?;

        let contents = self.local_signer.decrypt_event_contents(&event).await?;

        // Convert into a response
        let response: Nip46Response = serde_json::from_str(&contents)?;

        // Close the subscription
        self.client.close_subscription(sub_id).await?;

        Ok(response)
    }

    /// Disconnect from the relay
    pub async fn disconnect(&self) -> Result<(), Error> {
        *self.sub_id.write().unwrap() = None;
        self.client.disconnect().await
    }

    /// Disconnect from the relay and lock
    pub async fn disconnect_and_lock(&self) -> Result<(), Error> {
        *self.sub_id.write().unwrap() = None;
        self.client.disconnect().await?;
        self.local_signer.lock();
        Ok(())
    }
}

#[async_trait]
impl Signer for BunkerClient {
    fn public_key(&self) -> PublicKey {
        self.public_key
    }

    fn encrypted_private_key(&self) -> Option<EncryptedPrivateKey> {
        // NIP-46 does not offer an export function (yet)
        None
    }

    async fn sign_event(&self, pre_event: PreEvent) -> Result<Event, Error> {
        if self.is_locked() {
            return Err(Error::SignerIsLocked);
        }

        let pre_event_string = serde_json::to_string(&pre_event)?;
        let request = Nip46Request::new("sign_event".to_owned(), vec![pre_event_string]);
        let response = self.call(request).await?;
        if let Some(error) = response.error {
            if !error.is_empty() {
                return Err(Error::Nip46Error(error));
            }
        }
        let event: Event = serde_json::from_str(&response.result)?;
        Ok(event)
    }

    async fn encrypt(
        &self,
        other: &PublicKey,
        plaintext: &str,
        algo: ContentEncryptionAlgorithm,
    ) -> Result<String, Error> {
        if self.is_locked() {
            return Err(Error::SignerIsLocked);
        }

        let cmd = match algo {
            ContentEncryptionAlgorithm::Nip04 => "nip04_encrypt",
            ContentEncryptionAlgorithm::Nip44v1Unpadded => return Err(Error::UnsupportedAlgorithm),
            ContentEncryptionAlgorithm::Nip44v1Padded => return Err(Error::UnsupportedAlgorithm),
            ContentEncryptionAlgorithm::Nip44v2 => "nip44_encrypt",
        };

        let request = Nip46Request::new(
            cmd.to_owned(),
            vec![other.as_hex_string(), plaintext.to_owned()],
        );

        let response = self.call(request).await?;
        if let Some(error) = response.error {
            if !error.is_empty() {
                return Err(Error::Nip46Error(error));
            }
        }

        Ok(response.result)
    }

    async fn decrypt(&self, other: &PublicKey, ciphertext: &str) -> Result<String, Error> {
        if self.is_locked() {
            return Err(Error::SignerIsLocked);
        }

        let cmd = if ciphertext.contains("?iv=") {
            "nip04_decrypt"
        } else {
            "nip44_decrypt"
        };

        let request = Nip46Request::new(
            cmd.to_owned(),
            vec![other.as_hex_string(), ciphertext.to_owned()],
        );

        let response = self.call(request).await?;
        if let Some(error) = response.error {
            if !error.is_empty() {
                return Err(Error::Nip46Error(error));
            }
        }

        Ok(response.result)
    }

    fn key_security(&self) -> Result<KeySecurity, Error> {
        Ok(KeySecurity::NotTracked)
    }
}

use serde::de::Error as DeError;
use serde::de::{Deserialize, Deserializer, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeSeq, Serializer};
use std::fmt;

impl Serialize for BunkerClient {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(5))?;
        seq.serialize_element(&self.remote_signer_pubkey)?;
        seq.serialize_element(&self.relay_url)?;
        seq.serialize_element(&self.local_signer)?;
        seq.serialize_element(&self.public_key)?;
        seq.serialize_element(&self.timeout)?;
        seq.end()
    }
}

impl<'de> Deserialize<'de> for BunkerClient {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(BunkerClientVisitor)
    }
}

struct BunkerClientVisitor;

impl<'de> Visitor<'de> for BunkerClientVisitor {
    type Value = BunkerClient;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a serialized BunkerClient as a sequence")
    }

    fn visit_seq<A>(self, mut access: A) -> Result<BunkerClient, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let remote_signer_pubkey = access
            .next_element::<PublicKey>()?
            .ok_or_else(|| DeError::custom("Missing remote_signer_pubkey"))?;
        let relay_url = access
            .next_element::<RelayUrl>()?
            .ok_or_else(|| DeError::custom("Missing relay_url"))?;
        let local_signer = access
            .next_element::<Arc<KeySigner>>()?
            .ok_or_else(|| DeError::custom("Missing local_signer"))?;
        let public_key = access
            .next_element::<PublicKey>()?
            .ok_or_else(|| DeError::custom("Missing public_key"))?;
        let timeout = access
            .next_element::<Duration>()?
            .ok_or_else(|| DeError::custom("Missing timeout"))?;
        let client = client::Client::new(relay_url.as_str());
        Ok(BunkerClient {
            remote_signer_pubkey,
            relay_url,
            local_signer,
            public_key,
            timeout,
            client,
            sub_id: RwLock::new(None),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::PrivateKey;

    #[test]
    fn test_bunker_client_serde() {
        let prebunk = PreBunkerClient::new(
            PrivateKey::generate().public_key(),
            RelayUrl::try_from_str("wss://relay.example/").unwrap(),
            None,
            "password",
        )
        .unwrap();

        let s = serde_json::to_string(&prebunk).unwrap();
        println!("{s}");
        let prebunk2: PreBunkerClient = serde_json::from_str(&*s).unwrap();
        assert_eq!(prebunk.remote_signer_pubkey, prebunk2.remote_signer_pubkey);
        assert_eq!(prebunk.relay_url, prebunk2.relay_url);
        assert_eq!(prebunk.connect_secret, prebunk2.connect_secret);
        assert_eq!(
            prebunk.local_signer.encrypted_private_key(),
            prebunk2.local_signer.encrypted_private_key()
        );
    }
}
