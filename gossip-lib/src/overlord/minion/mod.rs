mod filter_fns;
mod handle_websocket;
mod subscription;
mod subscription_map;

use crate::comms::{ToMinionMessage, ToMinionPayload, ToMinionPayloadDetail, ToOverlordMessage};
use crate::dm_channel::DmChannel;
use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::relay::Relay;
use crate::{RunState, USER_AGENT};
use base64::Engine;
use encoding_rs::{Encoding, UTF_8};
use futures_util::sink::SinkExt;
use futures_util::stream::{FusedStream, StreamExt};
use http::uri::{Parts, Scheme};
use http::Uri;
use mime::Mime;
use nostr_types::{
    ClientMessage, EventAddr, EventKind, Filter, Id, IdHex, PreEvent, PublicKey, PublicKeyHex,
    RelayInformationDocument, RelayUrl, Tag, Unixtime,
};
use reqwest::Response;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::time::Duration;
use subscription_map::SubscriptionMap;
use tokio::net::TcpStream;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::watch::Receiver as WatchReceiver;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::protocol::{Message as WsMessage, WebSocketConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthState {
    None,
    Waiting(Id), // we sent AUTH, have not got response back yet
    Authenticated,
    Failed,
}

pub struct EventSeekState {
    pub job_ids: Vec<u64>,
    pub asked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinionExitReason {
    GotDisconnected,
    GotShutdownMessage,
    GotWSClose,
    LostOverlord,
    SubscriptionsHaveCompleted,
    Unknown,
}

impl MinionExitReason {
    pub fn benign(&self) -> bool {
        matches!(
            *self,
            MinionExitReason::GotShutdownMessage | MinionExitReason::SubscriptionsHaveCompleted
        )
    }
}

pub struct Minion {
    url: RelayUrl,
    to_overlord: UnboundedSender<ToOverlordMessage>,
    from_overlord: Receiver<ToMinionMessage>,
    dbrelay: Relay,
    nip11: Option<RelayInformationDocument>,
    stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    subscription_map: SubscriptionMap,
    next_events_subscription_id: u32,
    postings: HashSet<Id>,
    sought_events: HashMap<Id, EventSeekState>,
    last_message_sent: String,
    auth_challenge: String,
    subscriptions_waiting_for_auth: HashMap<String, Unixtime>,
    subscriptions_waiting_for_metadata: Vec<(u64, Vec<PublicKey>)>,
    subscriptions_rate_limited: Vec<String>,
    general_feed_keys: Vec<PublicKey>,
    general_feed_start: Option<Unixtime>,
    person_feed_start: Option<Unixtime>,
    inbox_feed_start: Option<Unixtime>,
    read_runstate: WatchReceiver<RunState>,
    exiting: Option<MinionExitReason>,
    auth_state: AuthState,
    failed_subs: HashSet<String>,
}

impl Minion {
    pub async fn new(url: RelayUrl) -> Result<Minion, Error> {
        let to_overlord = GLOBALS.to_overlord.clone();
        let from_overlord = GLOBALS.to_minions.subscribe();
        let dbrelay = GLOBALS.storage.read_or_create_relay(&url, None)?;

        let mut read_runstate = GLOBALS.read_runstate.clone();
        if *read_runstate.borrow_and_update() != RunState::Online {
            return Err(ErrorKind::Offline.into());
        }

        Ok(Minion {
            url,
            to_overlord,
            from_overlord,
            dbrelay,
            nip11: None,
            stream: None,
            subscription_map: SubscriptionMap::new(),
            next_events_subscription_id: 0,
            postings: HashSet::new(),
            sought_events: HashMap::new(),
            last_message_sent: String::new(),
            auth_challenge: "".to_string(),
            subscriptions_waiting_for_auth: HashMap::new(),
            subscriptions_waiting_for_metadata: Vec::new(),
            subscriptions_rate_limited: Vec::new(),
            general_feed_keys: Vec::new(),
            general_feed_start: None,
            person_feed_start: None,
            inbox_feed_start: None,
            read_runstate,
            exiting: None,
            auth_state: AuthState::None,
            failed_subs: HashSet::new(),
        })
    }
}

impl Minion {
    pub(crate) async fn handle(
        &mut self,
        mut messages: Vec<ToMinionPayload>,
    ) -> Result<MinionExitReason, Error> {
        // minion will log when it connects
        tracing::trace!("{}: Minion handling started", &self.url);

        // Possibly use a short timeout
        let mut short_timeout = false;
        for m in &messages {
            // When advertising relay lists, use a short timeout
            if matches!(m.detail, ToMinionPayloadDetail::AdvertiseRelayList(_)) {
                short_timeout = true;
            }
        }

        let fetcher_timeout = if short_timeout {
            std::time::Duration::new(5, 0)
        } else {
            std::time::Duration::new(GLOBALS.storage.read_setting_fetcher_timeout_sec(), 0)
        };

        // Connect to the relay
        let websocket_stream = {
            // Parse the URI
            let uri: http::Uri = self.url.as_str().parse::<Uri>()?;
            let mut parts: Parts = uri.into_parts();
            parts.scheme = match parts.scheme {
                Some(scheme) => match scheme.as_str() {
                    "wss" => Some(Scheme::HTTPS),
                    "ws" => Some(Scheme::HTTP),
                    _ => Some(Scheme::HTTPS),
                },
                None => Some(Scheme::HTTPS),
            };
            let uri = http::Uri::from_parts(parts)?;

            // Fetch NIP-11 data
            let request_nip11_future = reqwest::Client::builder()
                .timeout(fetcher_timeout)
                .redirect(reqwest::redirect::Policy::none())
                .gzip(true)
                .brotli(true)
                .deflate(true)
                .build()?
                .get(format!("{}", uri))
                .header("Accept", "application/nostr+json")
                .send();

            let response;
            tokio::select! {
                _ = self.read_runstate.wait_for(|runstate| !runstate.going_online()) => {
                    return Ok(MinionExitReason::GotShutdownMessage);
                },
                response_result = request_nip11_future => {
                    response = response_result?;
                }
            }

            self.dbrelay.last_attempt_nip11 = Some(Unixtime::now().unwrap().0 as u64);
            let status = response.status();
            match Self::text_with_charset(response, "utf-8").await {
                Ok(text) => {
                    if status.is_server_error() {
                        tracing::warn!(
                            "{}: {}",
                            &self.url,
                            status.canonical_reason().unwrap_or("")
                        );
                    } else {
                        match serde_json::from_str::<RelayInformationDocument>(&text) {
                            Ok(nip11) => {
                                tracing::debug!("{}: {}", &self.url, nip11);
                                self.nip11 = Some(nip11);
                                self.dbrelay.nip11 = self.nip11.clone();
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "{}: Unable to parse response as NIP-11 ({}): {}\n",
                                    &self.url,
                                    e,
                                    text.lines()
                                        .take(
                                            GLOBALS
                                                .storage
                                                .read_setting_nip11_lines_to_output_on_error()
                                        )
                                        .collect::<Vec<_>>()
                                        .join("\n")
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("{}: Unable to read NIP-11 response: {}", &self.url, e);
                }
            }

            // Save updated NIP-11 data (even if it failed)
            GLOBALS.storage.write_relay(&self.dbrelay, None)?;

            let key: [u8; 16] = rand::random();

            let req = http::request::Request::builder().method("GET");

            let req = if GLOBALS.storage.read_setting_set_user_agent() {
                req.header("User-Agent", USER_AGENT)
            } else {
                req
            };

            // Some relays want an Origin header to filter requests. Of course we
            // don't have an Origin, but whatever, for these specific relays we will
            // give them something.
            let req = if self.url.as_str() == "wss://relay.snort.social"
                || self.url.as_str() == "wss://relay-pub.deschooling.us"
            {
                // Like Damus, we will set it to the URL of the relay itself
                req.header("Origin", self.url.as_str())
            } else {
                req
            };

            let uri: http::Uri = self.url.as_str().parse::<Uri>()?;
            let host = uri.host().unwrap(); // fixme
            let req = req
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

            let config: WebSocketConfig = WebSocketConfig {
                // Tungstenite default is 64 MiB.
                // Cameri nostream relay limits to 0.5 a megabyte
                // Based on my current database of 7356 events, the longest was 11,121 bytes.
                // Cameri said people with >2k followers were losing data at 128kb cutoff.
                max_message_size: Some(
                    GLOBALS.storage.read_setting_max_websocket_message_size_kb() * 1024,
                ),
                max_frame_size: Some(
                    GLOBALS.storage.read_setting_max_websocket_frame_size_kb() * 1024,
                ),
                accept_unmasked_frames: GLOBALS
                    .storage
                    .read_setting_websocket_accept_unmasked_frames(),
                ..Default::default()
            };

            let connect_timeout_secs = if short_timeout {
                5
            } else {
                GLOBALS.storage.read_setting_websocket_connect_timeout_sec()
            };

            let connect_future = tokio::time::timeout(
                std::time::Duration::new(connect_timeout_secs, 0),
                tokio_tungstenite::connect_async_with_config(req, Some(config), false),
            );

            let websocket_stream;
            let response;
            tokio::select! {
                _ = self.read_runstate.wait_for(|runstate| !runstate.going_online()) => {
                    return Ok(MinionExitReason::GotShutdownMessage);
                },
                connect_result = connect_future => {
                    (websocket_stream, response) = connect_result??;
                },
            }

            // Check the status code of the response
            if response.status().as_u16() == 4000 {
                return Err(ErrorKind::RelayRejectedUs.into());
            }

            tracing::debug!("{}: Connected", &self.url);

            websocket_stream
        };

        self.stream = Some(websocket_stream);

        // Bump the success count for the relay
        self.bump_success_count(true).await;

        // Handle initial messages
        for message in messages.drain(..) {
            self.handle_overlord_message(message).await?;
        }

        // Ping timer
        let mut ping_timer = tokio::time::interval(std::time::Duration::new(
            GLOBALS.storage.read_setting_websocket_ping_frequency_sec(),
            0,
        ));
        ping_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        ping_timer.tick().await; // use up the first immediate tick.

        // Periodic Task timer (1.5 sec)
        let mut task_timer = tokio::time::interval(std::time::Duration::new(1, 500_000_000));
        task_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        task_timer.tick().await; // use up the first immediate tick.

        'relayloop: loop {
            match self.loop_handler(&mut ping_timer, &mut task_timer).await {
                Ok(_) => {
                    if self.exiting.is_some() {
                        break 'relayloop;
                    }
                }
                Err(e) => {
                    tracing::warn!("{}", e);

                    if let ErrorKind::Websocket(_) = e.kind {
                        return Err(e);
                    }

                    // for other errors, keep going
                }
            }
        }

        // Close the connection
        let ws_stream = self.stream.as_mut().unwrap();
        if !ws_stream.is_terminated() {
            if self.exiting != Some(MinionExitReason::GotWSClose) {
                if let Err(e) = ws_stream.send(WsMessage::Close(None)).await {
                    tracing::warn!("websocket close error: {}", e);
                    return Err(e.into());
                }
            }
        }

        match self.exiting {
            Some(reason) => {
                tracing::debug!("Minion for {} shutting down: {:?}", self.url, reason);
                Ok(reason)
            }
            None => {
                tracing::debug!("Minion for {} shutting down", self.url);
                Ok(MinionExitReason::Unknown)
            }
        }
    }

    async fn loop_handler(
        &mut self,
        ping_timer: &mut tokio::time::Interval,
        task_timer: &mut tokio::time::Interval,
    ) -> Result<(), Error> {
        let ws_stream = self.stream.as_mut().unwrap();

        tokio::select! {
            biased;
            _ = self.read_runstate.changed() => {
                // NOTE: I couldn't get .wait_for() to work because it made all this code not Send anymore.
                if self.read_runstate.borrow_and_update().going_offline() {
                    self.exiting = Some(MinionExitReason::GotShutdownMessage);
                }
            },
            _ = ping_timer.tick() => {
                ws_stream.send(WsMessage::Ping(vec![0x1])).await?;
            },
            _ = task_timer.tick()  => {
                // Update subscription for sought events
                self.get_events().await?;

                // Try to subscribe to subscriptions waiting for something
                self.try_subscribe_waiting().await?;
            },
            to_minion_message = self.from_overlord.recv() => {
                let to_minion_message = match to_minion_message {
                    Ok(m) => m,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        self.exiting = Some(MinionExitReason::LostOverlord);
                        return Ok(());
                    },
                    Err(e) => return Err(e.into())
                };
                if to_minion_message.target == self.url.as_str() || to_minion_message.target == "all" {
                    self.handle_overlord_message(to_minion_message.payload).await?;
                }
            },
            ws_message = ws_stream.next() => {
                let ws_message = match ws_message {
                    Some(m) => m,
                    None => {
                        if ws_stream.is_terminated() {
                            // possibly connection reset
                            tracing::info!("{}: connected terminated", &self.url);
                            self.exiting = Some(MinionExitReason::GotDisconnected);
                        }
                        return Ok(());
                    }
                }?;

                GLOBALS.bytes_read.fetch_add(ws_message.len(), Ordering::Relaxed);

                tracing::trace!("{}: Handling message", &self.url);
                match ws_message {
                    WsMessage::Text(t) => {
                        // MAYBE FIXME, spawn a separate task here so that
                        // we don't miss ping ticks
                        self.handle_nostr_message(t).await?;
                        // FIXME: some errors we should probably bail on.
                        // For now, try to continue.
                    },
                    WsMessage::Binary(_) => tracing::warn!("{}, Unexpected binary message", &self.url),
                    WsMessage::Ping(_) => { }, // tungstenite automatically pongs.
                    WsMessage::Pong(_) => { }, // Verify it is 0x1? Nah. It's just for keep-alive.
                    WsMessage::Close(_) => {
                        self.exiting = Some(MinionExitReason::GotWSClose);
                    }
                    WsMessage::Frame(_) => tracing::warn!("{}: Unexpected frame message", &self.url),
                }
            },
        }

        // Don't continue if we have no more subscriptions
        if self.subscription_map.is_empty() {
            self.exiting = Some(MinionExitReason::SubscriptionsHaveCompleted);
        }

        Ok(())
    }

    pub(crate) async fn handle_overlord_message(
        &mut self,
        message: ToMinionPayload,
    ) -> Result<(), Error> {
        match message.detail {
            ToMinionPayloadDetail::AdvertiseRelayList(event) => {
                let id = event.id;
                self.postings.insert(id);
                let msg = ClientMessage::Event(event);
                let wire = serde_json::to_string(&msg)?;
                let ws_stream = self.stream.as_mut().unwrap();
                self.last_message_sent = wire.clone();
                ws_stream.send(WsMessage::Text(wire)).await?;
                tracing::info!("Advertised relay list to {}", &self.url);
                self.to_overlord.send(ToOverlordMessage::MinionJobComplete(
                    self.url.clone(),
                    message.job_id,
                ))?;
            }
            ToMinionPayloadDetail::AuthApproved => {
                self.dbrelay.allow_auth = Some(true); // save in our memory copy of the relay
                self.authenticate().await?;
                if let Some(pubkey) = GLOBALS.identity.public_key() {
                    GLOBALS.pending.remove(
                        &crate::pending::PendingItem::RelayAuthenticationRequest {
                            account: pubkey,
                            relay: self.url.clone(),
                        },
                    );
                }
            }
            ToMinionPayloadDetail::AuthDeclined => {
                self.dbrelay.allow_auth = Some(false); // save in our memory copy of the relay
                if let Some(pubkey) = GLOBALS.identity.public_key() {
                    GLOBALS.pending.remove(
                        &crate::pending::PendingItem::RelayAuthenticationRequest {
                            account: pubkey,
                            relay: self.url.clone(),
                        },
                    );
                }
            }
            ToMinionPayloadDetail::FetchEvent(id) => {
                self.sought_events
                    .entry(id)
                    .and_modify(|ess| ess.job_ids.push(message.job_id))
                    .or_insert(EventSeekState {
                        job_ids: vec![message.job_id],
                        asked: false,
                    });
                // We don't ask the relay immediately. See task_timer.
            }
            ToMinionPayloadDetail::FetchEventAddr(ea) => {
                // These are rare enough we can ask immediately. We can't store in sought_events
                // anyways we would have to create a parallel thing.
                self.get_event_addr(message.job_id, ea).await?;
            }
            ToMinionPayloadDetail::PostEvents(mut events) => {
                for event in events.drain(..) {
                    let id = event.id;
                    self.postings.insert(id);
                    let msg = ClientMessage::Event(Box::new(event));
                    let wire = serde_json::to_string(&msg)?;
                    let ws_stream = self.stream.as_mut().unwrap();
                    self.last_message_sent = wire.clone();
                    ws_stream.send(WsMessage::Text(wire)).await?;
                    tracing::info!("Posted event to {}", &self.url);
                }
                self.to_overlord.send(ToOverlordMessage::MinionJobComplete(
                    self.url.clone(),
                    message.job_id,
                ))?;
            }
            ToMinionPayloadDetail::Shutdown => {
                tracing::debug!("{}: Websocket listener shutting down", &self.url);
                self.exiting = Some(MinionExitReason::GotShutdownMessage);
            }
            ToMinionPayloadDetail::SubscribeAugments(ids) => {
                self.subscribe_augments(message.job_id, ids).await?;
            }
            ToMinionPayloadDetail::SubscribeGeneralFeed(pubkeys) => {
                if self.general_feed_keys.is_empty() {
                    self.general_feed_keys = pubkeys;
                    self.subscribe_general_feed_initial(message.job_id).await?;
                } else {
                    self.subscribe_general_feed_additional(message.job_id, pubkeys)
                        .await?;
                }
            }
            ToMinionPayloadDetail::SubscribeInbox => {
                self.subscribe_inbox(message.job_id).await?;
            }
            ToMinionPayloadDetail::SubscribeConfig => {
                self.subscribe_config(message.job_id).await?;
            }
            ToMinionPayloadDetail::SubscribeDiscover(pubkeys) => {
                self.subscribe_discover(message.job_id, pubkeys).await?;
            }
            ToMinionPayloadDetail::SubscribePersonContactList(pubkey) => {
                self.subscribe_person_contactlist(message.job_id, pubkey)
                    .await?;
            }
            ToMinionPayloadDetail::SubscribePersonFeed(pubkey) => {
                self.subscribe_person_feed(message.job_id, pubkey).await?;
            }
            ToMinionPayloadDetail::SubscribeReplies(main) => {
                self.subscribe_replies(message.job_id, main).await?;
            }
            ToMinionPayloadDetail::SubscribeDmChannel(dmchannel) => {
                self.subscribe_dm_channel(message.job_id, dmchannel).await?;
            }
            ToMinionPayloadDetail::SubscribeNip46 => {
                self.subscribe_nip46(message.job_id).await?;
            }
            ToMinionPayloadDetail::TempSubscribeGeneralFeedChunk(start) => {
                self.temp_subscribe_general_feed_chunk(message.job_id, start)
                    .await?;
            }
            ToMinionPayloadDetail::TempSubscribePersonFeedChunk { pubkey, start } => {
                self.temp_subscribe_person_feed_chunk(message.job_id, pubkey, start)
                    .await?;
            }
            ToMinionPayloadDetail::TempSubscribeInboxFeedChunk(start) => {
                self.temp_subscribe_inbox_feed_chunk(message.job_id, start)
                    .await?;
            }
            ToMinionPayloadDetail::TempSubscribeMetadata(pubkeys) => {
                self.temp_subscribe_metadata(message.job_id, pubkeys)
                    .await?;
            }
            ToMinionPayloadDetail::UnsubscribePersonFeed => {
                self.unsubscribe("person_feed").await?;
            }
            ToMinionPayloadDetail::UnsubscribeReplies => {
                self.unsubscribe("replies").await?;
            }
        }

        Ok(())
    }

    async fn subscribe_augments(&mut self, job_id: u64, ids: Vec<IdHex>) -> Result<(), Error> {
        let filters = filter_fns::augments(&ids);

        self.subscribe(filters, "temp_augments", job_id).await?;

        if let Some(sub) = self.subscription_map.get_mut("temp_augments") {
            if let Some(nip11) = &self.nip11 {
                if !nip11.supports_nip(15) {
                    // Does not support EOSE.  Set subscription to EOSE now.
                    sub.set_eose();
                }
            } else {
                // Does not support EOSE.  Set subscription to EOSE now.
                sub.set_eose();
            }
        }

        Ok(())
    }

    // Subscribe to the user's followers on the relays they write to
    async fn subscribe_general_feed_initial(&mut self, job_id: u64) -> Result<(), Error> {
        tracing::debug!(
            "Following {} people at {}",
            self.general_feed_keys.len(),
            &self.url
        );

        // Compute how far to look back
        let since = {
            if self.general_feed_start.is_some() {
                // We already have a general subscription.
                // Therefore, don't lookback at all. We are just adding more people
                //    and we don't want to reload everybody's events again.
                // FIXME: we should do a separate temp subscription for the more people added
                // to get their events over the past chunk.
                Unixtime::now().unwrap()
            } else {
                let since = self.compute_since(GLOBALS.storage.read_setting_feed_chunk());
                self.general_feed_start = Some(since);
                since
            }
        };

        let filters = filter_fns::general_feed(&self.general_feed_keys, since, None);

        if filters.is_empty() {
            self.unsubscribe("general_feed").await?;
            self.to_overlord.send(ToOverlordMessage::MinionJobComplete(
                self.url.clone(),
                job_id,
            ))?;
        } else {
            self.subscribe(filters, "general_feed", job_id).await?;

            if let Some(sub) = self.subscription_map.get_mut("general_feed") {
                if let Some(nip11) = &self.nip11 {
                    if !nip11.supports_nip(15) {
                        // Does not support EOSE.  Set subscription to EOSE now.
                        sub.set_eose();
                    }
                } else {
                    // Does not support EOSE.  Set subscription to EOSE now.
                    sub.set_eose();
                }
            }
        }

        Ok(())
    }

    /// Subscribe to general feed with change of pubkeys
    async fn subscribe_general_feed_additional(
        &mut self,
        job_id: u64,
        pubkeys: Vec<PublicKey>,
    ) -> Result<(), Error> {
        // Figure out who the new people are (if any)
        let mut new_keys = pubkeys.clone();
        new_keys.retain(|key| !self.general_feed_keys.contains(key));
        if !new_keys.is_empty() {
            let since = match self.general_feed_start {
                Some(start) => start,
                None => self.compute_since(GLOBALS.storage.read_setting_feed_chunk()),
            };

            let filters = filter_fns::general_feed(&new_keys, since, None);

            if !filters.is_empty() {
                self.subscribe(filters, "temp_general_feed_update", job_id)
                    .await?;
            }
        }

        self.general_feed_keys = pubkeys;

        Ok(())
    }

    // Subscribe to anybody mentioning the user on the relays the user reads from
    // (and any other relay for the time being until nip65 is in widespread use)
    async fn subscribe_inbox(&mut self, job_id: u64) -> Result<(), Error> {
        // If we have already subscribed to inbox, do not resubscribe
        if self.subscription_map.has("inbox_feed") {
            return Ok(());
        }

        // Compute how far to look back
        let replies_since = self.compute_since(GLOBALS.storage.read_setting_replies_chunk());
        self.inbox_feed_start = Some(replies_since);

        let spamsafe = self.dbrelay.has_usage_bits(Relay::SPAMSAFE);

        let filters = filter_fns::inbox_feed(replies_since, None, spamsafe);

        if filters.is_empty() {
            return Ok(());
        }

        self.subscribe(filters, "inbox_feed", job_id).await?;

        if let Some(sub) = self.subscription_map.get_mut("inbox_feed") {
            if let Some(nip11) = &self.nip11 {
                if !nip11.supports_nip(15) {
                    // Does not support EOSE.  Set subscription to EOSE now.
                    sub.set_eose();
                }
            } else {
                // Does not support EOSE.  Set subscription to EOSE now.
                sub.set_eose();
            }
        }

        Ok(())
    }

    // Subscribe to the user's config (config, DMs, etc) which is on their own write relays
    async fn subscribe_config(&mut self, job_id: u64) -> Result<(), Error> {
        let since = self.compute_since(GLOBALS.storage.read_setting_person_feed_chunk());

        let filters = filter_fns::config(since);

        if filters.is_empty() {
            return Ok(());
        } else {
            self.subscribe(filters, "config_feed", job_id).await?;
        }

        Ok(())
    }

    // Discover relay lists
    async fn subscribe_discover(
        &mut self,
        job_id: u64,
        pubkeys: Vec<PublicKey>,
    ) -> Result<(), Error> {
        if !pubkeys.is_empty() {
            let filters = filter_fns::discover(&pubkeys);

            self.subscribe(filters, "temp_discover_feed", job_id)
                .await?;
        }

        Ok(())
    }

    // Subscribe to a person's contactlist which is on their own write relays
    async fn subscribe_person_contactlist(
        &mut self,
        job_id: u64,
        pubkey: PublicKey,
    ) -> Result<(), Error> {
        let pkh: PublicKeyHex = pubkey.into();

        // Read back in things that we wrote out to our write relays
        let filters: Vec<Filter> = vec![Filter {
            authors: vec![pkh.clone().into()],
            kinds: vec![EventKind::ContactList],
            // these are all replaceable, no since required
            ..Default::default()
        }];

        tracing::debug!("subscribe_person_contactlist pkh {}", pkh );

        self.subscribe(filters, "temp_person_contactlist", job_id)
            .await?;

        Ok(())
    }

    // Subscribe to the posts a person generates on the relays they write to
    async fn subscribe_person_feed(&mut self, job_id: u64, pubkey: PublicKey) -> Result<(), Error> {
        // NOTE we do not unsubscribe to the general feed

        let since = self.compute_since(GLOBALS.storage.read_setting_person_feed_chunk());
        self.person_feed_start = Some(since);

        let filters = filter_fns::person_feed(pubkey, since, None);

        if filters.is_empty() {
            self.unsubscribe_person_feed().await?;
            self.to_overlord.send(ToOverlordMessage::MinionJobComplete(
                self.url.clone(),
                job_id,
            ))?;
        } else {
            self.subscribe(filters, "person_feed", job_id).await?;
        }

        Ok(())
    }

    async fn temp_subscribe_person_feed_chunk(
        &mut self,
        job_id: u64,
        pubkey: PublicKey,
        since: Unixtime,
    ) -> Result<(), Error> {
        let until = match self.person_feed_start {
            Some(old_since_new_until) => old_since_new_until,
            None => Unixtime::now().unwrap(),
        };
        self.person_feed_start = Some(since);

        let filters = filter_fns::person_feed(pubkey, since, Some(until));

        if filters.is_empty() {
            self.unsubscribe_person_feed().await?;
            self.to_overlord.send(ToOverlordMessage::MinionJobComplete(
                self.url.clone(),
                job_id,
            ))?;
        } else {
            let sub_name = format!("temp_person_feed_chunk_{}", job_id);
            self.subscribe(filters, &sub_name, job_id).await?;
        }

        Ok(())
    }

    async fn temp_subscribe_inbox_feed_chunk(
        &mut self,
        job_id: u64,
        since: Unixtime,
    ) -> Result<(), Error> {
        let until = match self.inbox_feed_start {
            Some(old_since_new_until) => old_since_new_until,
            None => Unixtime::now().unwrap(),
        };
        self.inbox_feed_start = Some(since);

        let spamsafe = self.dbrelay.has_usage_bits(Relay::SPAMSAFE);

        let filters = filter_fns::inbox_feed(since, Some(until), spamsafe);

        if filters.is_empty() {
            self.to_overlord.send(ToOverlordMessage::MinionJobComplete(
                self.url.clone(),
                job_id,
            ))?;
            return Ok(());
        }

        let sub_name = format!("temp_inbox_feed_chunk_{}", job_id);
        self.subscribe(filters, &sub_name, job_id).await?;

        Ok(())
    }

    async fn unsubscribe_person_feed(&mut self) -> Result<(), Error> {
        // Unsubscribe person_feed and all person feed chunks
        let handles = self
            .subscription_map
            .get_all_handles_matching("person_feed");
        for handle in handles {
            self.unsubscribe(&handle).await?;
        }
        self.person_feed_start = None;
        Ok(())
    }

    async fn subscribe_replies(&mut self, job_id: u64, main: IdHex) -> Result<(), Error> {
        // NOTE we do not unsubscribe to the general feed

        // Replies
        let spamsafe = self.dbrelay.has_usage_bits(Relay::SPAMSAFE);
        let filters = filter_fns::replies(main, spamsafe);
        self.subscribe(filters, "replies", job_id).await?;

        Ok(())
    }

    async fn subscribe_dm_channel(
        &mut self,
        job_id: u64,
        dmchannel: DmChannel,
    ) -> Result<(), Error> {
        let filters = filter_fns::dm_channel(dmchannel);

        if !filters.is_empty() {
            self.subscribe(filters, "dm_channel", job_id).await?;
        }

        Ok(())
    }

    async fn subscribe_nip46(&mut self, job_id: u64) -> Result<(), Error> {
        let filters = filter_fns::nip46();

        if !filters.is_empty() {
            self.subscribe(filters, "nip46", job_id).await?;
        }

        Ok(())
    }

    async fn get_events(&mut self) -> Result<(), Error> {
        // Collect all the sought events we have not yet asked for, and
        // presumptively mark them as having been asked for.
        let mut ids: Vec<IdHex> = Vec::new();
        for (id, ess) in self.sought_events.iter_mut() {
            if !ess.asked {
                ids.push((*id).into());
                ess.asked = true;
            }
        }

        // Bail if nothing is sought
        if ids.is_empty() {
            return Ok(());
        }

        // The subscription job_id wont be used.
        let job_id: u64 = u64::MAX;

        // create the filter
        let mut filter = Filter::new();
        filter.ids = ids;

        tracing::trace!("{}: Event Filter: {} events", &self.url, filter.ids.len());

        // create a handle for ourselves
        // This is always a fresh subscription because they handle keeps changing
        let handle = format!("temp_events_{}", self.next_events_subscription_id);
        self.next_events_subscription_id += 1;

        self.subscribe(vec![filter], &handle, job_id).await?;

        Ok(())
    }

    async fn try_subscribe_waiting(&mut self) -> Result<(), Error> {
        // Subscribe to metadata
        if !self.subscription_map.has("temp_subscribe_metadata")
            && !self.subscriptions_waiting_for_metadata.is_empty()
        {
            let mut subscriptions_waiting_for_metadata =
                std::mem::take(&mut self.subscriptions_waiting_for_metadata);
            let mut combined_job_id: Option<u64> = None;
            let mut combined_pubkeys: Vec<PublicKey> = Vec::new();
            for (job_id, pubkeys) in subscriptions_waiting_for_metadata.drain(..) {
                if combined_job_id.is_none() {
                    combined_job_id = Some(job_id)
                } else {
                    // Tell the overlord this job id is over (it got combined into
                    // another job_id)
                    self.to_overlord.send(ToOverlordMessage::MinionJobComplete(
                        self.url.clone(),
                        job_id,
                    ))?;
                }
                combined_pubkeys.extend(pubkeys);
            }

            self.temp_subscribe_metadata(combined_job_id.unwrap(), combined_pubkeys)
                .await?;
        }

        // If we are authenticated
        if self.auth_state != AuthState::Authenticated {
            // Apply subscriptions that were waiting for auth
            let mut handles = std::mem::take(&mut self.subscriptions_waiting_for_auth);
            let now = Unixtime::now().unwrap();
            for (handle, when) in handles.drain() {
                // Do not try if we just inserted it within the last second
                if when - now < Duration::from_secs(1) {
                    // re-insert
                    self.subscriptions_waiting_for_auth.insert(handle, when);
                    continue;
                }

                tracing::info!("Sending corked subscription {} to {}", handle, &self.url);
                self.send_subscription(&handle).await?;
            }
        }

        // Retry rate-limited subscriptions
        if !self.subscriptions_rate_limited.is_empty() {
            let mut handles = std::mem::take(&mut self.subscriptions_rate_limited);
            for handle in handles.drain(..) {
                tracing::info!(
                    "Sending previously rate-limited subscription {} to {}",
                    handle,
                    &self.url
                );
                self.send_subscription(&handle).await?;
            }
        }

        Ok(())
    }

    async fn get_event_addr(&mut self, job_id: u64, ea: EventAddr) -> Result<(), Error> {
        // create a handle for ourselves
        let handle = format!("temp_event_addr_{}", self.next_events_subscription_id);
        self.next_events_subscription_id += 1;

        // build the filter
        let mut filter = Filter::new();
        let pkh: PublicKeyHex = ea.author.into();
        filter.authors = vec![pkh];
        filter.kinds = vec![ea.kind];
        filter.set_tag_values('d', vec![ea.d]);

        self.subscribe(vec![filter], &handle, job_id).await
    }

    // Load more, one more chunk back
    async fn temp_subscribe_general_feed_chunk(
        &mut self,
        job_id: u64,
        start: Unixtime,
    ) -> Result<(), Error> {
        let end = {
            if let Some(end) = self.general_feed_start {
                end
            } else {
                // This shouldn't happen, but if it does
                Unixtime::now().unwrap()
            }
        };
        self.general_feed_start = Some(start);

        let filters = filter_fns::general_feed(&self.general_feed_keys, start, Some(end));

        if !filters.is_empty() {
            tracing::debug!(
                "Following {} people at {}, from {} to {}",
                self.general_feed_keys.len(),
                &self.url,
                start,
                end
            );

            // We include the job_id so that if the user presses "load more" yet again,
            // the new chunk subscription doesn't clobber this subscription which might
            // not have run to completion yet.
            let sub_name = format!("temp_general_feed_chunk_{}", job_id);

            self.subscribe(filters, &sub_name, job_id).await?;
        }

        Ok(())
    }

    async fn temp_subscribe_metadata(
        &mut self,
        job_id: u64,
        pubkeys: Vec<PublicKey>,
    ) -> Result<(), Error> {
        if self.subscription_map.has("temp_subscribe_metadata") {
            // Save for later
            self.subscriptions_waiting_for_metadata
                .push((job_id, pubkeys));
            return Ok(());
        }

        tracing::trace!("Temporarily subscribing to metadata on {}", &self.url);

        let handle = "temp_subscribe_metadata".to_string();

        let filters = filter_fns::metadata(&pubkeys);

        self.subscribe(filters, &handle, job_id).await
    }

    async fn subscribe(
        &mut self,
        filters: Vec<Filter>,
        handle: &str,
        job_id: u64,
    ) -> Result<(), Error> {
        if filters.is_empty() {
            tracing::warn!("EMPTY FILTERS handle={} jobid={}", handle, job_id);
            return Ok(());
        }

        if self.failed_subs.contains(handle) {
            tracing::debug!(
                "{}: Avoiding resubscribing to a previously failed subscription: {}",
                &self.url,
                handle
            );
            return Ok(());
        }

        if let Some(sub) = self.subscription_map.get_mut(handle) {
            // Gratitously bump the EOSE as if the relay was finished, since it was
            // our fault the subscription is getting cut off.  This way we will pick up
            // where we left off instead of potentially loading a bunch of events
            // yet again.
            let now = Unixtime::now().unwrap();

            // Update last general EOSE
            self.dbrelay.last_general_eose_at = Some(match self.dbrelay.last_general_eose_at {
                Some(old) => old.max(now.0 as u64),
                None => now.0 as u64,
            });

            sub.set_filters(filters);
            let old_job_id = sub.change_job_id(job_id);
            let id = sub.get_id();
            tracing::debug!(
                "UPDATED SUBSCRIPTION on {} handle={}, id={}",
                &self.url,
                handle,
                id
            );
            self.to_overlord.send(ToOverlordMessage::MinionJobUpdated(
                self.url.clone(),
                old_job_id,
                job_id,
            ))?;
        } else {
            let id = self.subscription_map.add(handle, job_id, filters);
            tracing::debug!(
                "NEW SUBSCRIPTION on {} handle={}, id={}",
                &self.url,
                handle,
                &id
            );
        }

        if matches!(self.auth_state, AuthState::Waiting(_)) {
            // Save this, subscribe after AUTH completes
            self.subscriptions_waiting_for_auth
                .insert(handle.to_owned(), Unixtime::now().unwrap());
            return Ok(());
        }

        self.send_subscription(handle).await?;
        Ok(())
    }

    async fn send_subscription(&mut self, handle: &str) -> Result<(), Error> {
        let req_message = match self.subscription_map.get(handle) {
            Some(sub) => sub.req_message(),
            None => return Ok(()), // Not much we can do. It is not there.
        };
        let wire = serde_json::to_string(&req_message)?;
        let websocket_stream = self.stream.as_mut().unwrap();
        tracing::trace!("{}: Sending {}", &self.url, &wire);
        self.last_message_sent = wire.clone();
        websocket_stream.send(WsMessage::Text(wire.clone())).await?;
        Ok(())
    }

    async fn unsubscribe(&mut self, handle: &str) -> Result<(), Error> {
        if !self.subscription_map.has(handle) {
            return Ok(());
        }
        let subscription = self.subscription_map.get(handle).unwrap();
        let wire = serde_json::to_string(&subscription.close_message())?;
        let websocket_stream = self.stream.as_mut().unwrap();
        tracing::trace!("{}: Sending {}", &self.url, &wire);
        self.last_message_sent = wire.clone();
        websocket_stream.send(WsMessage::Text(wire.clone())).await?;
        let id = self.subscription_map.remove(handle);
        if let Some(id) = id {
            tracing::debug!(
                "END SUBSCRIPTION on {} handle={}, id={}",
                &self.url,
                handle,
                &id
            );
        } else {
            tracing::debug!(
                "END SUBSCRIPTION on {} handle={} NOT FOUND",
                &self.url,
                handle
            );
        }
        self.to_overlord.send(ToOverlordMessage::MinionJobComplete(
            self.url.clone(),
            subscription.get_job_id(),
        ))?;
        Ok(())
    }

    async fn authenticate(&mut self) -> Result<(), Error> {
        match self.auth_state {
            AuthState::Authenticated => return Ok(()),
            AuthState::Waiting(_) => return Ok(()),
            _ => (),
        }

        if !GLOBALS.identity.is_unlocked() {
            return Err(ErrorKind::NoPrivateKeyForAuth(self.url.clone()).into());
        }
        let pubkey = match GLOBALS.identity.public_key() {
            Some(pk) => pk,
            None => {
                return Err(ErrorKind::NoPrivateKeyForAuth(self.url.clone()).into());
            }
        };
        let pre_event = PreEvent {
            pubkey,
            created_at: Unixtime::now().unwrap(),
            kind: EventKind::Auth,
            tags: vec![
                Tag::new(&["relay", self.url.as_str()]),
                Tag::new(&["challenge", &self.auth_challenge]),
            ],
            content: "".to_string(),
        };
        let event = GLOBALS.identity.sign_event(pre_event)?;
        let id = event.id;
        let msg = ClientMessage::Auth(Box::new(event));
        let wire = serde_json::to_string(&msg)?;
        self.last_message_sent = wire.clone();
        let ws_stream = self.stream.as_mut().unwrap();
        ws_stream.send(WsMessage::Text(wire)).await?;
        tracing::info!("Authenticated to {}", &self.url);

        self.auth_state = AuthState::Waiting(id);

        Ok(())
    }

    // This replictes reqwest Response text_with_charset to handle decoding
    // whatever charset they used into UTF-8, as well as counting the bytes.
    async fn text_with_charset(
        response: Response,
        default_encoding: &str,
    ) -> Result<String, Error> {
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<Mime>().ok());
        let encoding_name = content_type
            .as_ref()
            .and_then(|mime| mime.get_param("charset").map(|charset| charset.as_str()))
            .unwrap_or(default_encoding);
        let encoding = Encoding::for_label(encoding_name.as_bytes()).unwrap_or(UTF_8);
        let full = response.bytes().await?;
        GLOBALS.bytes_read.fetch_add(full.len(), Ordering::Relaxed);
        let (text, _, _) = encoding.decode(&full);
        if let Cow::Owned(s) = text {
            return Ok(s);
        }
        unsafe {
            // decoding returned Cow::Borrowed, meaning these bytes
            // are already valid utf8
            Ok(String::from_utf8_unchecked(full.to_vec()))
        }
    }

    async fn bump_failure_count(&mut self) {
        // Update in self
        self.dbrelay.failure_count += 1;

        // Save to storage
        if let Err(e) = GLOBALS.storage.write_relay(&self.dbrelay, None) {
            tracing::error!("{}: ERROR bumping relay failure count: {}", &self.url, e);
        }
    }

    async fn bump_success_count(&mut self, also_bump_last_connected: bool) {
        let now = Unixtime::now().unwrap().0 as u64;

        // Update in self
        self.dbrelay.success_count += 1;
        if also_bump_last_connected {
            self.dbrelay.last_connected_at = Some(now);
        }

        // Save to storage
        if let Err(e) = GLOBALS.storage.write_relay(&self.dbrelay, None) {
            tracing::error!("{}: ERROR bumping relay success count: {}", &self.url, e);
        }
    }

    fn compute_since(&self, chunk_seconds: u64) -> Unixtime {
        let now = Unixtime::now().unwrap();
        let overlap = Duration::from_secs(GLOBALS.storage.read_setting_overlap());
        let chunk = Duration::from_secs(chunk_seconds);

        // FIXME - general subscription EOSE is not necessarily applicable to
        //         other subscriptions. BUt we don't record when we got an EOSE
        //         on other subscriptions.
        let eose: Unixtime = match self.dbrelay.last_general_eose_at {
            Some(u) => Unixtime(u as i64),
            None => Unixtime(0),
        };

        let mut since = eose;
        since = since - overlap;

        // No dates before 2020:
        if since.0 < 1577836800 {
            since.0 = 1577836800;
        }

        // Do not go back by more than one chunk
        let one_chunk_ago = now - chunk;

        since.max(one_chunk_ago)
    }
}
