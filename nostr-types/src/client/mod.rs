use crate::{
    ClientMessage, Error, Event, EventKind, Filter, Id, PreEvent, RelayInformationDocument,
    RelayMessage, Signer, SubscriptionId, Tag, Unixtime,
};
use http::Uri;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tungstenite::protocol::Message;

mod auth;
pub use auth::AuthState;

mod connection;
pub use connection::ClientConnection;

/// A client connection to a relay.
#[derive(Debug)]
pub struct Client {
    // read-only URL of the remote relay
    relay_url: String,

    // The connection information
    // We only write-lock this to create or disconnect. Normal operations are
    // read-locked with multiple readers allowed at once.
    connection: RwLock<Option<ClientConnection>>,
}

impl Client {
    /// Connect to a relay
    pub fn new(relay_url: &str) -> Client {
        Client {
            relay_url: relay_url.to_string(),
            connection: RwLock::new(None),
        }
    }

    /// Is connected
    pub async fn is_connected(&self) -> bool {
        if let Some(ref cc) = *self.connection.read().await {
            !cc.is_disconnected()
        } else {
            false
        }
    }

    /// Reconnect to the relay if needed
    async fn maybe_reconnect(&self, reconnect_timeout: Duration) -> Result<(), Error> {
        let maybe_data = if let Some(ref cc) = *self.connection.read().await {
            if cc.is_disconnected() {
                Some(cc.incoming())
            } else {
                return Ok(());
            }
        } else {
            None
        };

        match maybe_data {
            Some(data) => {
                let new_cc =
                    ClientConnection::new_with_data(&self.relay_url, reconnect_timeout, data)
                        .await?;
                *self.connection.write().await = Some(new_cc);
            }
            None => {
                let cc = ClientConnection::new(&self.relay_url, reconnect_timeout).await?;
                *self.connection.write().await = Some(cc);
            }
        }

        Ok(())
    }

    /// Disconnect from the relay
    pub async fn disconnect(&self) -> Result<(), Error> {
        let cc = std::mem::take(self.connection.write().await.deref_mut());
        if let Some(cc) = cc {
            cc.disconnect().await?
        }
        Ok(())
    }

    /// Get auth state
    pub async fn get_auth_state(&self) -> Result<AuthState, Error> {
        let lock = self.connection.read().await;
        let Some(ref cc) = *lock else {
            return Err(Error::Disconnected);
        };
        Ok(cc.get_auth_state().await)
    }

    /// Wait for auth state
    pub async fn wait_for_auth_state_change(&self, timeout: Duration) -> Result<AuthState, Error> {
        let lock = self.connection.read().await;
        let Some(ref cc) = *lock else {
            return Err(Error::Disconnected);
        };
        cc.wait_for_auth_state_change(timeout).await
    }

    /// Authenticate
    /// This does not wait for any reply.
    pub async fn send_authenticate(
        &self,
        challenge: String,
        signer: Arc<dyn Signer>,
        reconnect_timeout: Duration,
    ) -> Result<Id, Error> {
        let pre_event = PreEvent {
            pubkey: signer.public_key(),
            created_at: Unixtime::now(),
            kind: EventKind::Auth,
            tags: vec![
                Tag::new(&["relay", &self.relay_url]),
                Tag::new(&["challenge", &challenge]),
            ],
            content: "".to_string(),
        };
        let event = signer.sign_event(pre_event).await?;
        let id = event.id;
        self.maybe_reconnect(reconnect_timeout).await?;
        let lock = self.connection.read().await;
        let Some(ref cc) = *lock else {
            return Err(Error::Disconnected);
        };
        cc.send_authenticate(event).await?;
        Ok(id)
    }

    /// Full Authentication process.
    /// Run this when you get "auth-required" in an OK or CLOSED message if you
    /// wish to authenticate.
    pub async fn full_authenticate(
        &self,
        signer: Arc<dyn Signer>,
        timeout: Duration,
    ) -> Result<(), Error> {
        match self.get_auth_state().await? {
            AuthState::NotYetRequested => Err(Error::RelayDidNotAuth),
            AuthState::Challenged(ch) => {
                let _ = self.send_authenticate(ch, signer, timeout).await?;
                let auth_state = self.wait_for_auth_state_change(timeout).await?;
                match auth_state {
                    AuthState::Success => Ok(()),
                    AuthState::Failure(_) => Err(Error::RelayRejectedAuth),
                    _ => Err(Error::InvalidState(
                        "AuthState in unexpected state".to_owned(),
                    )),
                }
            }
            AuthState::InProgress(_id) => {
                let auth_state = self.wait_for_auth_state_change(timeout).await?;
                match auth_state {
                    AuthState::Success => Ok(()),
                    AuthState::Failure(_) => Err(Error::RelayRejectedAuth),
                    _ => Err(Error::InvalidState(
                        "AuthState in unexpected state".to_owned(),
                    )),
                }
            }
            AuthState::Success => Err(Error::RelayForgotAuth),
            AuthState::Failure(_) => Err(Error::RelayRejectedPost),
        }
    }

    /// Post an event to the relay
    pub async fn post_event(&self, event: Event, reconnect_timeout: Duration) -> Result<(), Error> {
        let message = ClientMessage::Event(Box::new(event));
        self.maybe_reconnect(reconnect_timeout).await?;
        let lock = self.connection.read().await;
        let Some(ref cc) = *lock else {
            return Err(Error::Disconnected);
        };
        cc.send_message(message).await?;
        Ok(())
    }

    /// Post a raw event to the relay
    pub async fn post_raw_event(
        &self,
        json: String,
        reconnect_timeout: Duration,
    ) -> Result<(), Error> {
        let wire = format!("[\"EVENT\",{}]", json);
        let msg = Message::Text(wire.into());
        self.maybe_reconnect(reconnect_timeout).await?;
        let lock = self.connection.read().await;
        let Some(ref cc) = *lock else {
            return Err(Error::Disconnected);
        };
        cc.send_ws_message(msg).await?;
        Ok(())
    }

    /// This posts the event, and waits for the OK result, authenticating
    /// if requested if auth is Some.
    pub async fn post_event_and_wait_for_result(
        &self,
        event: Event,
        timeout: Duration,
        auth: Option<Arc<dyn Signer>>,
    ) -> Result<(bool, String), Error> {
        self.post_event(event.clone(), timeout).await?;
        let (ok, why) = self.wait_for_ok(event.id, timeout).await?;
        if !ok && why.starts_with("auth-required:") {
            match auth {
                None => Err(Error::RelayRequiresAuth),
                Some(signer) => {
                    self.full_authenticate(signer, timeout).await?;
                    self.post_event(event.clone(), timeout).await?;
                    let (ok, why) = self.wait_for_ok(event.id, timeout).await?;
                    Ok((ok, why))
                }
            }
        } else {
            Ok((ok, why))
        }
    }

    /// Wait for an Ok
    pub async fn wait_for_ok(&self, id: Id, timeout: Duration) -> Result<(bool, String), Error> {
        let lock = self.connection.read().await;
        let Some(ref cc) = *lock else {
            return Err(Error::Disconnected);
        };
        let rm = cc
            .wait_for_relay_message(
                |rm| matches!(rm, RelayMessage::Ok(i, _, _) if *i==id),
                timeout,
            )
            .await?;
        match rm {
            RelayMessage::Ok(_, ok, msg) => Ok((ok, msg)),
            _ => unreachable!(),
        }
    }

    /// Subscribe to a filter. This does not wait for results.
    pub async fn subscribe(
        &self,
        filter: Filter,
        reconnect_timeout: Duration,
    ) -> Result<SubscriptionId, Error> {
        self.maybe_reconnect(reconnect_timeout).await?;
        let lock = self.connection.read().await;
        let Some(ref cc) = *lock else {
            return Err(Error::Disconnected);
        };
        cc.subscribe(filter).await
    }

    /// Close a subscription
    pub async fn close_subscription(&self, sub_id: SubscriptionId) -> Result<(), Error> {
        let lock = self.connection.read().await;
        let Some(ref cc) = *lock else {
            return Err(Error::Disconnected);
        };
        cc.close_subscription(sub_id).await
    }

    /// Wait for an event on the given subscription
    pub async fn wait_for_subscribed_event(
        &self,
        sub_id: SubscriptionId,
        timeout: Duration,
    ) -> Result<Event, Error> {
        let lock = self.connection.read().await;
        let Some(ref cc) = *lock else {
            return Err(Error::Disconnected);
        };
        let rm = cc
            .wait_for_relay_message(
                |rm| matches!(rm, RelayMessage::Event(s, _) if *s==sub_id),
                timeout,
            )
            .await?;
        match rm {
            RelayMessage::Event(_, event) => Ok(*event),
            _ => unreachable!(),
        }
    }

    /// Wait for an event on the given subscription
    pub async fn wait_for_subscribed_event_or_eose(
        &self,
        sub_id: SubscriptionId,
        timeout: Duration,
    ) -> Result<Option<Event>, Error> {
        let lock = self.connection.read().await;
        let Some(ref cc) = *lock else {
            return Err(Error::Disconnected);
        };
        let rm = cc
            .wait_for_relay_message(
                |rm| {
                    matches!(rm, RelayMessage::Event(s, _) if *s==sub_id)
                        || matches!(rm, RelayMessage::Eose(s) if *s==sub_id)
                },
                timeout,
            )
            .await?;
        match rm {
            RelayMessage::Event(_, event) => Ok(Some(*event)),
            RelayMessage::Eose(_) => Ok(None),
            _ => unreachable!(),
        }
    }

    /// Subscribe and collect all results up to the EOSE
    pub async fn subscribe_and_wait_for_events(
        &self,
        filter: Filter,
        timeout: Duration,
        signer: Option<Arc<dyn Signer>>,
    ) -> Result<Vec<Event>, Error> {
        let mut output: Vec<Event> = Vec::new();

        let mut sub_id = self.subscribe(filter.clone(), timeout).await?;

        let lock = self.connection.read().await;
        let Some(ref cc) = *lock else {
            return Err(Error::Disconnected);
        };

        loop {
            // Wait for any of EVENT or EOSE or CLOSED on this subscription_id
            let rm = cc
                .wait_for_relay_message(
                    |rm| {
                        matches!(rm, RelayMessage::Event(sid, _) if *sid==sub_id)
                            || matches!(rm, RelayMessage::Eose(sid) if *sid==sub_id)
                            || matches!(rm, RelayMessage::Closed(sid, _) if *sid==sub_id)
                    },
                    timeout,
                )
                .await?;

            match rm {
                RelayMessage::Event(_, event) => output.push(*event),
                RelayMessage::Eose(_) => return Ok(output),
                RelayMessage::Closed(_, message) => {
                    if message.starts_with("auth-required:") {
                        match signer {
                            Some(ref signer) => {
                                self.full_authenticate(signer.clone(), timeout).await?;
                                sub_id = self.subscribe(filter.clone(), timeout).await?;
                                continue;
                            }
                            None => {
                                return Err(Error::RelayRequiresAuth);
                            }
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}

fn url_to_host_and_uri(url: &str) -> Result<(String, Uri), Error> {
    let uri: http::Uri = url.parse::<http::Uri>()?;
    let authority = match uri.authority() {
        Some(auth) => auth.as_str(),
        None => return Err(Error::Url(url.to_string())),
    };
    let host = authority
        .find('@')
        .map(|idx| authority.split_at(idx + 1).1)
        .unwrap_or_else(|| authority);
    if host.is_empty() {
        Err(Error::Url(url.to_string()))
    } else {
        Ok((host.to_owned(), uri))
    }
}

/// Fetch a NIP-11 for a relay
pub async fn fetch_nip11(relay_url: &str) -> Result<RelayInformationDocument, Error> {
    use reqwest::redirect::Policy;
    use reqwest::Client;
    use std::time::Duration;

    let (host, uri) = url_to_host_and_uri(relay_url)?;
    let scheme = match uri.scheme() {
        Some(refscheme) => match refscheme.as_str() {
            "wss" => "https",
            "ws" => "http",
            u => panic!("Unknown scheme {}", u),
        },
        None => panic!("Relay URL has no scheme."),
    };
    let url = format!("{}://{}{}", scheme, host, uri.path());
    let client = Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(60))
        .timeout(Duration::from_secs(60))
        .connection_verbose(true)
        .build()?;
    let response = client
        .get(url)
        .header("Host", host)
        .header("Accept", "application/nostr+json")
        .send()
        .await?;
    let json = response.text().await?;
    let rid: RelayInformationDocument = serde_json::from_str(&json)?;
    Ok(rid)
}
