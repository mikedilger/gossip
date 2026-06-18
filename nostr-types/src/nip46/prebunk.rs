use crate::client::Client;
use crate::nip46::{BunkerClient, Nip46ConnectionParameters, Nip46Request, Nip46Response};
use crate::{Error, EventKind, Filter, KeySigner, LockableSigner, PublicKey, RelayUrl, Signer};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{event, span, Level};

/// This is a Remote Signer setup that has not yet discovered the user's PublicKey
/// As a result, it cannot implement Signer yet.
#[derive(Debug, Serialize, Deserialize)]
pub struct PreBunkerClient {
    /// The pubkey of the bunker
    pub remote_signer_pubkey: PublicKey,

    /// The relay the bunker is listening at
    pub relay_url: RelayUrl,

    /// The connect secret
    pub connect_secret: Option<String>,

    /// Our local identity
    pub local_signer: Arc<KeySigner>,

    /// Timeout
    pub timeout: Duration,
}

impl PreBunkerClient {
    /// Create a new BunkerClient, generating a fresh local identity
    pub fn new(
        remote_signer_pubkey: PublicKey,
        relay_url: RelayUrl,
        connect_secret: Option<String>,
        new_password: &str,
        timeout: Duration,
    ) -> Result<PreBunkerClient, Error> {
        let local_signer = Arc::new(KeySigner::generate(new_password, 18)?);

        Ok(PreBunkerClient {
            remote_signer_pubkey,
            relay_url,
            connect_secret,
            local_signer,
            timeout,
        })
    }

    /// Create a new nip46 client from a URL.
    ///
    /// This connects to the relay, but does not contact the bunker yet. Use `connect()` to
    /// initiate contact with the bunker.
    pub fn new_from_url(
        url: &str,
        new_password: &str,
        timeout: Duration,
    ) -> Result<PreBunkerClient, Error> {
        let Nip46ConnectionParameters {
            remote_signer_pubkey,
            relays,
            secret,
        } = Nip46ConnectionParameters::from_str(url)?;

        let local_signer = Arc::new(KeySigner::generate(new_password, 18)?);

        Ok(PreBunkerClient {
            remote_signer_pubkey,
            relay_url: relays[0].clone(),
            connect_secret: secret,
            local_signer,
            timeout,
        })
    }

    /// Is the signer locked?
    pub fn is_locked(&self) -> bool {
        self.local_signer.is_locked()
    }

    /// Unlock (if locked)
    pub fn unlock(&mut self, password: &str) -> Result<(), Error> {
        self.local_signer.unlock(password)
    }

    /// Connect to the relay and bunker, learn our user's PublicKey,
    /// and return a full BunkerClient which impl Signer
    pub async fn initialize(&mut self) -> Result<BunkerClient, Error> {
        let span = span!(Level::DEBUG, "nip46 Prebunk initializing");
        let _enter = span.enter();

        let client = Client::new(self.relay_url.as_str());

        let connect_response = {
            let connect_request = {
                let params = {
                    let mut params = vec![self.remote_signer_pubkey.as_hex_string()];
                    if let Some(secret) = &self.connect_secret {
                        params.push(secret.to_owned());
                    }
                    params
                };

                Nip46Request::new("connect".to_string(), params)
            };

            event!(Level::DEBUG, "Calling with connect request event");
            self.call(&client, connect_request).await?
        };

        if let Some(error) = connect_response.error {
            if !error.is_empty() {
                return Err(Error::Nip46Error(error));
            }
        }

        // Ask for our pubkey
        let pubkey_response = {
            let pubkey_request = {
                let params = vec![];
                Nip46Request::new("get_public_key".to_string(), params)
            };

            event!(Level::DEBUG, "Calling with pubkey request event");
            self.call(&client, pubkey_request).await?
        };

        // Verify there is no error
        if let Some(error) = pubkey_response.error {
            if !error.is_empty() {
                return Err(Error::Nip46Error(error));
            }
        }

        let public_key = PublicKey::try_from_hex_string(&pubkey_response.result, true)?;

        Ok(BunkerClient {
            remote_signer_pubkey: self.remote_signer_pubkey,
            relay_url: self.relay_url.clone(),
            local_signer: self.local_signer.clone(),
            public_key,
            timeout: self.timeout,
            client,
            sub_id: std::sync::RwLock::new(None),
        })
    }

    async fn call(&self, client: &Client, request: Nip46Request) -> Result<Nip46Response, Error> {
        let span = span!(Level::DEBUG, "nip46 Prebunk callfn");
        let _enter = span.enter();

        let event = request
            .to_event(self.remote_signer_pubkey, self.local_signer.clone())
            .await?;

        // Subscribe
        let mut filter = Filter::new();
        filter.add_author(self.remote_signer_pubkey);
        filter.add_event_kind(EventKind::NostrConnect);
        filter.add_tag_value('p', self.local_signer.public_key().as_hex_string());
        filter.limit = Some(1);
        event!(
            Level::DEBUG,
            "calling client subscribe to subscribe to responses from the remote signer"
        );
        let sub_id = client.subscribe(filter.clone(), self.timeout).await?;

        // Post event to server
        let event_id = event.id;
        event!(Level::DEBUG, "posting our event");
        client.post_event(event, self.timeout).await?;
        event!(Level::DEBUG, "waiting for OK response");
        let (ok, msg) = client.wait_for_ok(event_id, self.timeout).await?;
        if !ok {
            return Err(Error::Nip46FailedToPost(msg));
        }

        // Wait for a response on the subscription
        event!(
            Level::DEBUG,
            "waiting for a response event on the remote signer subscription"
        );
        let event = client
            .wait_for_subscribed_event(sub_id.clone(), self.timeout)
            .await?;
        let contents = self.local_signer.decrypt_event_contents(&event).await?;

        // Convert into a response
        let response: Nip46Response = serde_json::from_str(&contents)?;

        // Unsubscribe
        client.close_subscription(sub_id).await?;

        Ok(response)
    }
}
