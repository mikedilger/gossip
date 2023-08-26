mod handle_websocket;
mod subscription;
mod subscription_map;

use crate::comms::{ToMinionMessage, ToMinionPayload, ToMinionPayloadDetail, ToOverlordMessage};
use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::relay::Relay;
use crate::USER_AGENT;
use base64::Engine;
use encoding_rs::{Encoding, UTF_8};
use futures_util::sink::SinkExt;
use futures_util::stream::{FusedStream, StreamExt};
use http::uri::{Parts, Scheme};
use http::Uri;
use mime::Mime;
use nostr_types::{
    ClientMessage, EventKind, Filter, Id, IdHex, IdHexPrefix, PublicKey, PublicKeyHex,
    PublicKeyHexPrefix, RelayInformationDocument, RelayUrl, Unixtime,
};
use reqwest::Response;
use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::Duration;
use subscription_map::SubscriptionMap;
use tokio::net::TcpStream;
use tokio::select;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::protocol::{Message as WsMessage, WebSocketConfig};

pub struct Minion {
    url: RelayUrl,
    to_overlord: UnboundedSender<ToOverlordMessage>,
    from_overlord: Receiver<ToMinionMessage>,
    dbrelay: Relay,
    nip11: Option<RelayInformationDocument>,
    stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    subscription_map: SubscriptionMap,
    next_events_subscription_id: u32,
    keepgoing: bool,
    postings: HashSet<Id>,
}

impl Minion {
    pub async fn new(url: RelayUrl) -> Result<Minion, Error> {
        let to_overlord = GLOBALS.to_overlord.clone();
        let from_overlord = GLOBALS.to_minions.subscribe();
        let dbrelay = match GLOBALS.storage.read_relay(&url)? {
            Some(dbrelay) => dbrelay,
            None => {
                let dbrelay = Relay::new(url.clone());
                GLOBALS.storage.write_relay(&dbrelay, None)?;
                dbrelay
            }
        };

        Ok(Minion {
            url,
            to_overlord,
            from_overlord,
            dbrelay,
            nip11: None,
            stream: None,
            subscription_map: SubscriptionMap::new(),
            next_events_subscription_id: 0,
            keepgoing: true,
            postings: HashSet::new(),
        })
    }
}

impl Minion {
    pub async fn handle(&mut self, mut messages: Vec<ToMinionPayload>) -> Result<(), Error> {
        // minion will log when it connects
        tracing::trace!("{}: Minion handling started", &self.url);

        let fetcher_timeout =
            std::time::Duration::new(GLOBALS.storage.read_setting_fetcher_timeout_sec(), 0);

        // Connect to the relay
        let websocket_stream = {
            // Parse the URI
            let uri: http::Uri = self.url.0.parse::<Uri>()?;
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
            let response = request_nip11_future.await?;
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
            let req = if self.url.0 == "wss://relay.snort.social"
                || self.url.0 == "wss://relay-pub.deschooling.us"
            {
                // Like Damus, we will set it to the URL of the relay itself
                req.header("Origin", &self.url.0)
            } else {
                req
            };

            let uri: http::Uri = self.url.0.parse::<Uri>()?;
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
                max_send_queue: None,
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
            };

            let connect_timeout = GLOBALS.storage.read_setting_websocket_connect_timeout_sec();
            let (websocket_stream, response) = tokio::time::timeout(
                std::time::Duration::new(connect_timeout, 0),
                tokio_tungstenite::connect_async_with_config(req, Some(config), false),
            )
            .await??;

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

        // Tell the overlord we are ready to receive commands
        self.tell_overlord_we_are_ready().await?;

        'relayloop: loop {
            match self.loop_handler().await {
                Ok(_) => {
                    if !self.keepgoing {
                        break 'relayloop;
                    }
                }
                Err(e) => {
                    tracing::error!("{}", e);

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
            if let Err(e) = ws_stream.send(WsMessage::Close(None)).await {
                tracing::error!("websocket close error: {}", e);
                return Err(e.into());
            }
        }

        Ok(())
    }

    async fn loop_handler(&mut self) -> Result<(), Error> {
        let ws_stream = self.stream.as_mut().unwrap();

        let mut timer = tokio::time::interval(std::time::Duration::new(
            GLOBALS.storage.read_setting_websocket_ping_frequency_sec(),
            0,
        ));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        timer.tick().await; // use up the first immediate tick.

        select! {
            biased;
            _ = timer.tick() => {
                ws_stream.send(WsMessage::Ping(vec![0x1])).await?;
            },
            to_minion_message = self.from_overlord.recv() => {
                let to_minion_message = match to_minion_message {
                    Ok(m) => m,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        self.keepgoing = false;
                        return Ok(());
                    },
                    Err(e) => return Err(e.into())
                };
                if to_minion_message.target == self.url.0 || to_minion_message.target == "all" {
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
                            self.keepgoing = false;
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
                    WsMessage::Close(_) => self.keepgoing = false,
                    WsMessage::Frame(_) => tracing::warn!("{}: Unexpected frame message", &self.url),
                }
            },
        }

        // Don't continue if we have no more subscriptions
        if self.subscription_map.is_empty() {
            self.keepgoing = false;
        }

        Ok(())
    }

    pub async fn handle_overlord_message(&mut self, message: ToMinionPayload) -> Result<(), Error> {
        match message.detail {
            ToMinionPayloadDetail::FetchEvent(id) => {
                self.get_event(message.job_id, id).await?;
            }
            ToMinionPayloadDetail::PostEvent(event) => {
                let id = event.id;
                self.postings.insert(id);
                let msg = ClientMessage::Event(event);
                let wire = serde_json::to_string(&msg)?;
                let ws_stream = self.stream.as_mut().unwrap();
                ws_stream.send(WsMessage::Text(wire)).await?;
                tracing::info!("Posted event to {}", &self.url);
                self.to_overlord.send(ToOverlordMessage::MinionJobComplete(
                    self.url.clone(),
                    message.job_id,
                ))?;
            }
            ToMinionPayloadDetail::PullFollowing => {
                self.pull_following(message.job_id).await?;
            }
            ToMinionPayloadDetail::Shutdown => {
                tracing::debug!("{}: Websocket listener shutting down", &self.url);
                self.keepgoing = false;
            }
            ToMinionPayloadDetail::SubscribeAugments(ids) => {
                self.subscribe_augments(message.job_id, ids).await?;
            }
            ToMinionPayloadDetail::SubscribeGeneralFeed(pubkeys) => {
                self.subscribe_general_feed(message.job_id, pubkeys).await?;
            }
            ToMinionPayloadDetail::SubscribeMentions => {
                self.subscribe_mentions(message.job_id).await?;
            }
            ToMinionPayloadDetail::SubscribeConfig => {
                self.subscribe_config(message.job_id).await?;
            }
            ToMinionPayloadDetail::SubscribeDiscover(pubkeys) => {
                self.subscribe_discover(message.job_id, pubkeys).await?;
            }
            ToMinionPayloadDetail::SubscribePersonFeed(pubkey) => {
                self.subscribe_person_feed(message.job_id, pubkey).await?;
            }
            ToMinionPayloadDetail::SubscribeThreadFeed(main, parents) => {
                self.subscribe_thread_feed(message.job_id, main, parents)
                    .await?;
            }
            ToMinionPayloadDetail::TempSubscribeMetadata(pubkeys) => {
                self.temp_subscribe_metadata(message.job_id, pubkeys)
                    .await?;
            }
            ToMinionPayloadDetail::UnsubscribePersonFeed => {
                self.unsubscribe("person_feed").await?;
            }
            ToMinionPayloadDetail::UnsubscribeThreadFeed => {
                self.unsubscribe("thread_feed").await?;
            }
        }

        Ok(())
    }

    async fn tell_overlord_we_are_ready(&self) -> Result<(), Error> {
        self.to_overlord.send(ToOverlordMessage::MinionIsReady)?;
        Ok(())
    }

    async fn subscribe_augments(&mut self, job_id: u64, ids: Vec<IdHex>) -> Result<(), Error> {
        let mut event_kinds = crate::feed::feed_related_event_kinds(false);
        event_kinds.retain(|f| f.augments_feed_related());

        let filter = Filter {
            e: ids,
            kinds: event_kinds,
            ..Default::default()
        };

        self.subscribe(vec![filter], "temp_augments", job_id)
            .await?;

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
    async fn subscribe_general_feed(
        &mut self,
        job_id: u64,
        followed_pubkeys: Vec<PublicKey>,
    ) -> Result<(), Error> {
        let mut filters: Vec<Filter> = Vec::new();
        let overlap = Duration::from_secs(GLOBALS.storage.read_setting_overlap());
        let feed_chunk = Duration::from_secs(GLOBALS.storage.read_setting_feed_chunk());

        tracing::debug!(
            "Following {} people at {}",
            followed_pubkeys.len(),
            &self.url
        );

        // Compute how far to look back
        let feed_since = {
            let now = Unixtime::now().unwrap();

            if self.subscription_map.has("general_feed") {
                // don't lookback if we are just adding more people
                now
            } else {
                // Start with where we left off, the time we last got something from
                // this relay.
                let mut feed_since: Unixtime = match self.dbrelay.last_general_eose_at {
                    Some(u) => Unixtime(u as i64),
                    None => Unixtime(0),
                };

                // Subtract overlap to avoid gaps due to clock sync and event
                // propagation delay
                feed_since = feed_since - overlap;

                // Some relays don't like dates before 1970.  Hell, we don't need anything before 2020:
                if feed_since.0 < 1577836800 {
                    feed_since.0 = 1577836800;
                }

                let one_feedchunk_ago = now - feed_chunk;
                feed_since.max(one_feedchunk_ago)
            }
        };

        // Allow all feed related event kinds (excluding DMs)
        let event_kinds = crate::feed::feed_related_event_kinds(false);

        if let Some(pubkey) = GLOBALS.signer.public_key() {
            // feed related by me
            // FIXME copy this to listening to my write relays
            let pkh: PublicKeyHex = pubkey.into();
            filters.push(Filter {
                authors: vec![pkh.into()],
                kinds: event_kinds.clone(),
                since: Some(feed_since),
                ..Default::default()
            });
        }

        if !followed_pubkeys.is_empty() {
            let pkp: Vec<PublicKeyHexPrefix> = followed_pubkeys
                .iter()
                .map(|pk| Into::<PublicKeyHex>::into(*pk).prefix(16)) // quarter-size
                .collect();

            // feed related by people followed
            filters.push(Filter {
                authors: pkp,
                kinds: event_kinds.clone(),
                since: Some(feed_since),
                ..Default::default()
            });

            // Try to find where people post.
            // Subscribe to kind-10002 `RelayList`s to see where people post.
            // Subscribe to ContactLists so we can look at the contents and
            //   divine relays people write to (if using a client that does that).
            // BUT ONLY for people where this kind of data hasn't been received
            // in the last 8 hours (so we don't do it every client restart).
            let keys_needing_relay_lists: Vec<PublicKeyHexPrefix> = GLOBALS
                .people
                .get_followed_pubkeys_needing_relay_lists(&followed_pubkeys)
                .drain(..)
                .map(|pk| Into::<PublicKeyHex>::into(pk).prefix(16)) // quarter-size
                .collect();

            if !keys_needing_relay_lists.is_empty() {
                tracing::debug!(
                    "Looking to update relay lists from {} people on {}",
                    keys_needing_relay_lists.len(),
                    &self.url
                );

                filters.push(Filter {
                    authors: keys_needing_relay_lists,
                    kinds: vec![EventKind::RelayList, EventKind::ContactList],
                    // No since. These are replaceable events, we should only get 1 per person.
                    ..Default::default()
                });
            }
        }

        // NO REPLIES OR ANCESTORS

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

    // Subscribe to anybody mentioning the user on the relays the user reads from
    // (and any other relay for the time being until nip65 is in widespread use)
    async fn subscribe_mentions(&mut self, job_id: u64) -> Result<(), Error> {
        let mut filters: Vec<Filter> = Vec::new();
        let overlap = Duration::from_secs(GLOBALS.storage.read_setting_overlap());
        let replies_chunk = Duration::from_secs(GLOBALS.storage.read_setting_replies_chunk());

        // Compute how far to look back
        let replies_since = {
            // Start with where we left off, the time we last got something from
            // this relay.
            let mut replies_since: Unixtime = match self.dbrelay.last_general_eose_at {
                Some(u) => Unixtime(u as i64),
                None => Unixtime(0),
            };

            // Subtract overlap to avoid gaps due to clock sync and event
            // propagation delay
            replies_since = replies_since - overlap;

            // Some relays don't like dates before 1970.  Hell, we don't need anything before 2020:
            if replies_since.0 < 1577836800 {
                replies_since.0 = 1577836800;
            }

            let one_replieschunk_ago = Unixtime::now().unwrap() - replies_chunk;
            replies_since.max(one_replieschunk_ago)
        };

        // GiftWrap lookback needs to be one week further back
        // FIXME: this depends on how far other clients backdate.
        let giftwrap_since = Unixtime(replies_since.0 - 60 * 60 * 24 * 7);

        // Allow all feed related event kinds (including DMs)
        let mut event_kinds = crate::feed::feed_related_event_kinds(true);
        event_kinds.retain(|f| *f != EventKind::GiftWrap); // gift wrap has special filter

        if let Some(pubkey) = GLOBALS.signer.public_key() {
            // Any mentions of me
            // (but not in peoples contact lists, for example)

            let pkh: PublicKeyHex = pubkey.into();

            filters.push(Filter {
                p: vec![pkh.clone()],
                kinds: event_kinds,
                since: Some(replies_since),
                ..Default::default()
            });

            // Giftwrap specially looks back further
            filters.push(Filter {
                p: vec![pkh],
                kinds: vec![EventKind::GiftWrap],
                since: Some(giftwrap_since),
                ..Default::default()
            });
        }

        self.subscribe(filters, "mentions_feed", job_id).await?;

        if let Some(sub) = self.subscription_map.get_mut("mentions_feed") {
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

    // Subscribe to the user's config which is on their own write relays
    async fn subscribe_config(&mut self, job_id: u64) -> Result<(), Error> {
        if let Some(pubkey) = GLOBALS.signer.public_key() {
            let pkh: PublicKeyHex = pubkey.into();

            let filters: Vec<Filter> = vec![Filter {
                authors: vec![pkh.into()],
                kinds: vec![
                    EventKind::Metadata,
                    //EventKind::RecommendRelay,
                    EventKind::ContactList,
                    EventKind::RelayList,
                ],
                // these are all replaceable, no since required
                ..Default::default()
            }];

            self.subscribe(filters, "temp_config_feed", job_id).await?;
        }

        Ok(())
    }

    // Subscribe to the user's config which is on their own write relays
    async fn subscribe_discover(
        &mut self,
        job_id: u64,
        pubkeys: Vec<PublicKey>,
    ) -> Result<(), Error> {
        if !pubkeys.is_empty() {
            let pkp: Vec<PublicKeyHexPrefix> = pubkeys
                .iter()
                .map(|pk| Into::<PublicKeyHex>::into(*pk).prefix(16))
                .collect(); // quarter-size prefix

            let filters: Vec<Filter> = vec![Filter {
                authors: pkp,
                kinds: vec![EventKind::RelayList],
                // these are all replaceable, no since required
                ..Default::default()
            }];

            self.subscribe(filters, "temp_discover_feed", job_id)
                .await?;
        }

        Ok(())
    }

    // Subscribe to the posts a person generates on the relays they write to
    async fn subscribe_person_feed(&mut self, job_id: u64, pubkey: PublicKey) -> Result<(), Error> {
        // NOTE we do not unsubscribe to the general feed

        // Allow all feed related event kinds (excluding DMs)
        let event_kinds = crate::feed::feed_displayable_event_kinds(false);

        let filters: Vec<Filter> = vec![Filter {
            authors: vec![Into::<PublicKeyHex>::into(pubkey).prefix(16)],
            kinds: event_kinds,
            // No since, just a limit on quantity of posts
            limit: Some(25),
            ..Default::default()
        }];

        // let feed_chunk = GLOBALS.storage.read_setting_feed_chunk();

        // Don't do this anymore. It's low value and we can't compute how far back to look
        // until after we get their 25th oldest post.
        /*
        // Reactions to posts made by the person
        // (presuming people include a 'p' tag in their reactions)
        filters.push(Filter {
            kinds: vec![EventKind::Reaction],
            p: vec![pubkey],
            // Limited in time just so we aren't overwhelmed. Generally we only need reactions
            // to their most recent posts. This is a very sloppy and approximate solution.
            since: Some(Unixtime::now().unwrap() - Duration::from_secs(feed_chunk * 25)),
            ..Default::default()
        });
         */

        // persons metadata
        // we don't display this stuff in their feed, so probably take this out.
        // FIXME TBD
        /*
        filters.push(Filter {
            authors: vec![pubkey],
            kinds: vec![EventKind::Metadata, EventKind::RecommendRelay, EventKind::ContactList, EventKind::RelaysList],
            since: // last we last checked
            .. Default::default()
        });
         */

        // NO REPLIES OR ANCESTORS

        if filters.is_empty() {
            self.unsubscribe("person_feed").await?;
            self.to_overlord.send(ToOverlordMessage::MinionJobComplete(
                self.url.clone(),
                job_id,
            ))?;
        } else {
            self.subscribe(filters, "person_feed", job_id).await?;
        }

        Ok(())
    }

    async fn subscribe_thread_feed(
        &mut self,
        job_id: u64,
        main: IdHex,
        vec_ids: Vec<IdHex>,
    ) -> Result<(), Error> {
        // NOTE we do not unsubscribe to the general feed

        let mut filters: Vec<Filter> = Vec::new();

        if !vec_ids.is_empty() {
            let idhp: Vec<IdHexPrefix> = vec_ids
                .iter()
                .map(
                    |id| id.prefix(16), // quarter-size
                )
                .collect();

            // Get ancestors we know of so far
            filters.push(Filter {
                ids: idhp,
                ..Default::default()
            });

            // Get reactions to ancestors, but not replies
            let kinds = crate::feed::feed_augment_event_kinds();
            filters.push(Filter {
                e: vec_ids,
                kinds,
                ..Default::default()
            });
        }

        // Allow all feed related event kinds (excluding DMs)
        let event_kinds = crate::feed::feed_related_event_kinds(false);

        filters.push(Filter {
            e: vec![main],
            kinds: event_kinds,
            ..Default::default()
        });

        self.subscribe(filters, "thread_feed", job_id).await?;

        Ok(())
    }

    // Create or replace the following subscription
    /*
    async fn upsert_following(&mut self, pubkeys: Vec<PublicKeyHex>) -> Result<(), Error> {
        let websocket_stream = self.stream.as_mut().unwrap();

        if pubkeys.is_empty() {
            if let Some(sub) = self.subscription_map.get("following") {
                // Close the subscription
                let wire = serde_json::to_string(&sub.close_message())?;
                websocket_stream.send(WsMessage::Text(wire.clone())).await?;

                // Remove the subscription from the map
                self.subscription_map.remove("following");
            }

            // Since pubkeys is empty, nothing to subscribe to.
            return Ok(());
        }

        // Compute how far to look back
        let (feed_since, special_since) = {
        // Get related settings
            let overlap = Duration::from_secs(GLOBALS.storage.read_setting_overlap());
            let feed_chunk = Duration::from_secs(GLOBALS.storage.read_setting_feed_chunk());

            /*
            // Find the oldest 'last_fetched' among the 'person_relay' table.
            // Null values will come through as 0.
            let mut special_since: i64 =
                PersonRelay::fetch_oldest_last_fetched(&pubkeys, &self.url.0).await? as i64;
            */

            // Start with where we left off, the time we last got something from
            // this relay.
            let mut special_since: i64 = match self.dbrelay.last_general_eose_at {
                Some(u) => u as i64,
                None => 0,
            };

            // Subtract overlap to avoid gaps due to clock sync and event
            // propagation delay
            special_since -= overlap as i64;

            // Some relays don't like dates before 1970.  Hell, we don't need anything before 2020:a
            if special_since < 1577836800 {
                special_since = 1577836800;
            }

            // For feed related events, don't look back more than one feed_chunk ago
            let one_feedchunk_ago = Unixtime::now().unwrap().0 - feed_chunk as i64;
            let feed_since = special_since.max(one_feedchunk_ago);

            (Unixtime(feed_since), Unixtime(special_since))
        };

        // Create the author filter
        let mut feed_filter: Filter = Filter::new();
        for pk in pubkeys.iter() {
            feed_filter.add_author(pk, None);
        }
        feed_filter.add_event_kind(EventKind::TextNote);
        feed_filter.add_event_kind(EventKind::Reaction);
        feed_filter.add_event_kind(EventKind::EventDeletion);
        feed_filter.since = Some(feed_since);

        tracing::trace!(
            "{}: Feed Filter: {} authors",
            &self.url,
            feed_filter.authors.len()
        );

        // Create the lookback filter
        let mut special_filter: Filter = Filter::new();
        for pk in pubkeys.iter() {
            special_filter.add_author(pk, None);
        }
        special_filter.add_event_kind(EventKind::Metadata);
        special_filter.add_event_kind(EventKind::RecommendRelay);
        special_filter.add_event_kind(EventKind::ContactList);
        special_filter.add_event_kind(EventKind::RelaysList);
        special_filter.since = Some(special_since);

        tracing::trace!(
            "{}: Special Filter: {} authors",
            &self.url,
            special_filter.authors.len()
        );

        // Get the subscription
        let req_message = if self.subscription_map.has("following") {
            let sub = self.subscription_map.get_mut("following").unwrap();
            let vec: &mut Vec<Filter> = sub.get_mut();
            vec.clear();
            vec.push(feed_filter);
            vec.push(special_filter);
            sub.req_message()
        } else {
            self.subscription_map
                .add("following", vec![feed_filter, special_filter]);
            self.subscription_map.get("following").unwrap().req_message()
        };

        // Subscribe (or resubscribe) to the subscription
        let wire = serde_json::to_string(&req_message)?;
        websocket_stream.send(WsMessage::Text(wire.clone())).await?;

        tracing::trace!("{}: Sent {}", &self.url, &wire);

        Ok(())
    }
     */

    async fn get_event(&mut self, job_id: u64, id: IdHex) -> Result<(), Error> {
        // create the filter
        let mut filter = Filter::new();
        filter.ids = vec![id.into()];

        tracing::trace!("{}: Event Filter: {} events", &self.url, filter.ids.len());

        // create a handle for ourselves
        let handle = format!("temp_events_{}", self.next_events_subscription_id);
        self.next_events_subscription_id += 1;

        // save the subscription
        let id = self.subscription_map.add(&handle, job_id, vec![filter]);
        tracing::debug!(
            "NEW SUBSCRIPTION on {} handle={}, id={}",
            &self.url,
            handle,
            &id
        );

        // get the request message
        let req_message = self.subscription_map.get(&handle).unwrap().req_message();

        // Subscribe on the relay
        let websocket_stream = self.stream.as_mut().unwrap();
        let wire = serde_json::to_string(&req_message)?;
        websocket_stream.send(WsMessage::Text(wire.clone())).await?;

        tracing::trace!("{}: Sent {}", &self.url, &wire);

        Ok(())
    }

    async fn temp_subscribe_metadata(
        &mut self,
        job_id: u64,
        mut pubkeys: Vec<PublicKey>,
    ) -> Result<(), Error> {
        let pkhp: Vec<PublicKeyHexPrefix> = pubkeys
            .drain(..)
            .map(
                |pk| Into::<PublicKeyHex>::into(pk).prefix(16), // quarter-size
            )
            .collect();

        tracing::trace!("Temporarily subscribing to metadata on {}", &self.url);

        let handle = "temp_subscribe_metadata".to_string();
        let filter = Filter {
            authors: pkhp,
            kinds: vec![EventKind::Metadata],
            // FIXME: we could probably get a since-last-fetched-their-metadata here.
            //        but relays should just return the lastest of these.
            ..Default::default()
        };
        self.subscribe(vec![filter], &handle, job_id).await
    }

    async fn pull_following(&mut self, job_id: u64) -> Result<(), Error> {
        if let Some(pubkey) = GLOBALS.signer.public_key() {
            let pkh: PublicKeyHex = pubkey.into();
            let filter = Filter {
                authors: vec![pkh.into()],
                kinds: vec![EventKind::ContactList],
                ..Default::default()
            };
            self.subscribe(vec![filter], "temp_following_list", job_id)
                .await?;
        }
        Ok(())
    }

    async fn subscribe(
        &mut self,
        filters: Vec<Filter>,
        handle: &str,
        job_id: u64,
    ) -> Result<(), Error> {
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

        let req_message = self.subscription_map.get(handle).unwrap().req_message();
        let wire = serde_json::to_string(&req_message)?;
        let websocket_stream = self.stream.as_mut().unwrap();
        tracing::trace!("{}: Sending {}", &self.url, &wire);
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
}
