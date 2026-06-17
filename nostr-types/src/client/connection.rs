use super::AuthState;
use crate::{ClientMessage, Error, Event, Filter, RelayMessage, SubscriptionId};
use base64::Engine;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Notify, RwLock};
use tracing::{event, span, Level};
use tungstenite::protocol::Message;

/// A WebSocket
type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// A live connection to a relay, and all related state.
///
/// This connects when created, but may persist beyond disconnection, so we can't say that
/// it is always connected. Reconnection is not done here; if it becomes disconnected and you
/// want to reconnect then you should drop and recreate to reconnect (and probably take the
/// Incoming data to not lose it)
#[derive(Debug)]
pub struct ClientConnection {
    // Send messages with this
    sink: Mutex<SplitSink<Ws, Message>>,

    // Keeps subscription ids unique
    next_sub_id: AtomicUsize,

    // Authentication data
    auth_state: Arc<RwLock<AuthState>>,

    // The listener (stream) task handle
    // listener_task: JoinHandle<()>,

    // Incoming messages deposited by the listener task
    incoming: Arc<RwLock<Vec<RelayMessage>>>,

    // A signal that a new message has arrived, OR
    // that the connection has been closed, OR
    // that the authentication state has changed
    wake: Arc<Notify>,

    // Disconnection data
    disconnected: Arc<AtomicBool>,
}

impl ClientConnection {
    /// Create a new ClientConnection by connecting.
    pub async fn new(relay_url: &str, timeout: Duration) -> Result<ClientConnection, Error> {
        let incoming: Arc<RwLock<Vec<RelayMessage>>> = Arc::new(RwLock::new(Vec::new()));
        Self::new_with_data(relay_url, timeout, incoming).await
    }

    /// Create a new ClientConnectdion by connecting, preserving data from a previous connection.
    pub async fn new_with_data(
        relay_url: &str,
        timeout: Duration,
        incoming: Arc<RwLock<Vec<RelayMessage>>>,
    ) -> Result<ClientConnection, Error> {
        let (host, uri) = super::url_to_host_and_uri(relay_url)?;
        let key: [u8; 16] = rand::random();
        let request = http::request::Request::builder()
            .method("GET")
            .header("Host", host)
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header(
                "Sec-WebSocket-Key",
                base64::engine::general_purpose::STANDARD.encode(key),
            )
            .uri(uri)
            .body(())?;

        let (websocket, response) =
            tokio::time::timeout(timeout, tokio_tungstenite::connect_async(request)).await??;

        let status = response.status();
        if status.is_redirection() || status.is_client_error() || status.is_server_error() {
            return Err(Error::WebsocketConnectionFailed(status));
        }

        // Split the websocket
        let (sink, mut stream) = websocket.split();

        let incoming2 = incoming.clone();

        let disconnected: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let disconnected2 = disconnected.clone();

        let wake = Arc::new(Notify::new());
        let wake2 = wake.clone();

        let auth_state = Arc::new(RwLock::new(AuthState::NotYetRequested));
        let auth_state2 = auth_state.clone();

        // Start a task to handle the incoming stream
        let _listener_task = tokio::task::spawn(Box::pin(async move {
            let span = span!(Level::DEBUG, "connection listener thread");
            let _enter = span.enter();
            while let Some(message) = stream.next().await {
                match message {
                    Ok(Message::Text(s)) => {
                        match serde_json::from_str(&s) {
                            Ok(rm) => {
                                // Maybe update authentication state
                                match rm {
                                    RelayMessage::Auth(challenge) => {
                                        let mut lock = auth_state2.write().await;
                                        match *lock {
                                            AuthState::NotYetRequested => {
                                                *lock = AuthState::Challenged(challenge)
                                            }
                                            _ => {
                                                event!(Level::DEBUG, "dup auth ignored");
                                                continue; // dup auth ignored
                                            }
                                        }
                                        // No need to store into incoming
                                        event!(Level::DEBUG, "waking (received AUTH)");
                                        wake2.notify_waiters();
                                        continue;
                                    }
                                    RelayMessage::Ok(id, is_ok, ref reason) => {
                                        let mut lock = auth_state2.write().await;
                                        if let AuthState::InProgress(sent_id) = *lock {
                                            if id == sent_id {
                                                *lock = if is_ok {
                                                    AuthState::Success
                                                } else {
                                                    AuthState::Failure(reason.clone())
                                                };
                                                // No need to store into incoming
                                                event!(Level::DEBUG, "waking (received OK)");
                                                wake2.notify_waiters();
                                                continue;
                                            }
                                        }
                                    }
                                    RelayMessage::Closed(_, _) => {
                                        event!(Level::DEBUG, "waking (received CLOSED)");
                                    }
                                    RelayMessage::Eose(_) => {
                                        event!(Level::DEBUG, "waking (received EOSE)");
                                    }
                                    RelayMessage::Event(_, _) => {
                                        event!(Level::DEBUG, "waking (received EVENT)");
                                    }
                                    RelayMessage::Notice(ref n) => {
                                        event!(Level::DEBUG, "waking (received NOTICE: {n})");
                                    }
                                    RelayMessage::Notify(ref n) => {
                                        event!(Level::DEBUG, "waking (received NOTIFY: {n})");
                                    }
                                    RelayMessage::Count(_, _) => {
                                        event!(Level::DEBUG, "waking (received COUNT)");
                                    }
                                }

                                (*incoming2.write().await).push(rm);
                                wake2.notify_waiters();
                            }
                            Err(e) => {
                                event!(Level::DEBUG, "websocket wessage failed to deserialize: {e}")
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        event!(Level::DEBUG, "remote websocket is closing the connection");
                        break;
                    }
                    Ok(m) => {
                        event!(Level::DEBUG, "unhandled websocket message kind: {m:?}");
                    }
                    Err(e) => {
                        event!(Level::ERROR, "{e}");
                        break;
                    }
                }
            }

            disconnected2.store(true, Ordering::Relaxed);
        }));

        Ok(ClientConnection {
            sink: Mutex::new(sink),
            next_sub_id: AtomicUsize::new(0),
            auth_state,
            incoming,
            wake,
            disconnected,
        })
    }

    /// Is disconnected
    pub fn is_disconnected(&self) -> bool {
        self.disconnected.load(Ordering::Relaxed)
    }

    /// Disconnect from the relay, consuming self
    pub async fn disconnect(self) -> Result<(), Error> {
        let msg = Message::Close(None);
        let mut sink = self.sink.lock().await;
        sink.send(msg).await?;
        sink.close().await?;
        Ok(())
    }

    /// Copy an Arc reference to the Incoming relay messages.
    pub fn incoming(&self) -> Arc<RwLock<Vec<RelayMessage>>> {
        self.incoming.clone()
    }

    fn fail_if_disconnected(&self) -> Result<(), Error> {
        if self.disconnected.load(Ordering::Relaxed) {
            Err(Error::Disconnected)
        } else {
            Ok(())
        }
    }

    /// Subscribe to a filter. This does not wait for results.
    pub async fn subscribe(&self, filter: Filter) -> Result<SubscriptionId, Error> {
        self.fail_if_disconnected()?;
        let sub_id_usize = self.next_sub_id.fetch_add(1, Ordering::Relaxed);
        let sub_id = SubscriptionId(format!("sub{}", sub_id_usize));
        let client_message = ClientMessage::Req(sub_id.clone(), filter.clone());
        self.send_message(client_message).await?;
        Ok(sub_id)
    }

    /// Close a subscription
    pub async fn close_subscription(&self, sub_id: SubscriptionId) -> Result<(), Error> {
        self.fail_if_disconnected()?;
        let client_message = ClientMessage::Close(sub_id);
        self.send_message(client_message).await?;
        Ok(())
    }

    /// Send a `ClientMessage`
    pub async fn send_message(&self, message: ClientMessage) -> Result<(), Error> {
        let wire = serde_json::to_string(&message)?;
        let msg = Message::Text(wire.into());
        self.inner_send_message(msg).await?;
        Ok(())
    }

    /// Send a websocket `Message`
    pub async fn send_ws_message(&self, message: Message) -> Result<(), Error> {
        self.inner_send_message(message).await?;
        Ok(())
    }

    async fn inner_send_message(&self, msg: Message) -> Result<(), Error> {
        self.fail_if_disconnected()?;
        if let Err(e) = self.sink.lock().await.send(msg).await {
            self.disconnected.store(true, Ordering::Relaxed);
            Err(e)?
        } else {
            Ok(())
        }
    }

    /// Wait for some matching RelayMessage.
    ///
    /// The timeout will be reset when any event happens, so it make take
    /// longer than the timeout to give up.
    pub async fn wait_for_relay_message<P>(
        &self,
        predicate: P,
        timeout: Duration,
    ) -> Result<RelayMessage, Error>
    where
        P: Fn(&RelayMessage) -> bool,
    {
        let giveup = Instant::now() + timeout;

        loop {
            {
                // Check incoming for a match
                let mut incoming = self.incoming.write().await;
                if let Some(found) = incoming.iter().position(&predicate) {
                    let relay_message = incoming.remove(found);
                    return Ok(relay_message);
                }
            }

            if Instant::now() > giveup {
                return Err(Error::TimedOut);
            }

            // Wait for something to happen, or timeout
            tokio::time::timeout(timeout, self.wake.notified()).await?;

            self.fail_if_disconnected()?;
        }
    }

    /// Get AuthState
    pub async fn get_auth_state(&self) -> AuthState {
        self.auth_state.read().await.clone()
    }

    /// Wait for the given AuthState to occur.
    pub async fn wait_for_auth_state_change(&self, timeout: Duration) -> Result<AuthState, Error> {
        let start = self.auth_state.read().await.clone();
        loop {
            let current = self.auth_state.read().await.clone();
            if current != start {
                return Ok(current);
            }

            // Wait for something to happen, or timeout
            tokio::time::timeout(timeout, self.wake.notified()).await?;

            self.fail_if_disconnected()?;
        }
    }

    /// Authenticate
    pub async fn send_authenticate(&self, event: Event) -> Result<(), Error> {
        *self.auth_state.write().await = AuthState::InProgress(event.id);
        self.send_message(ClientMessage::Auth(Box::new(event)))
            .await?;
        Ok(())
    }
}
