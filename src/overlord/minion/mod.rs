mod handle_websocket;
mod subscription;

use crate::comms::{ToMinionMessage, ToMinionPayload, ToOverlordMessage};
use crate::db::DbRelay;
use crate::error::Error;
use crate::globals::GLOBALS;
use crate::USER_AGENT;
use base64::Engine;
use encoding_rs::{Encoding, UTF_8};
use futures::{SinkExt, StreamExt};
use futures_util::stream::{SplitSink, SplitStream};
use http::uri::{Parts, Scheme};
use http::Uri;
use mime::Mime;
use nostr_types::{
    ClientMessage, EventKind, Filter, IdHex, IdHexPrefix, PublicKeyHex, PublicKeyHexPrefix,
    RelayInformationDocument, RelayUrl, Unixtime,
};
use reqwest::Response;
use std::borrow::Cow;
use std::sync::atomic::Ordering;
use std::time::Duration;
use subscription::Subscriptions;
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
    dbrelay: DbRelay,
    nip11: Option<RelayInformationDocument>,
    stream: Option<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>,
    sink: Option<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, WsMessage>>,
    subscriptions: Subscriptions,
    next_events_subscription_id: u32,
    keepgoing: bool,
}

impl Minion {
    pub async fn new(url: RelayUrl) -> Result<Minion, Error> {
        let to_overlord = GLOBALS.to_overlord.clone();
        let from_overlord = GLOBALS.to_minions.subscribe();
        let dbrelay = match DbRelay::fetch_one(&url).await? {
            Some(dbrelay) => dbrelay,
            None => {
                let dbrelay = DbRelay::new(url.clone());
                DbRelay::insert(dbrelay.clone()).await?;
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
            sink: None,
            subscriptions: Subscriptions::new(),
            next_events_subscription_id: 0,
            keepgoing: true,
        })
    }
}

impl Minion {
    pub async fn handle(&mut self) {
        // Catch errors, Return nothing.
        if let Err(e) = self.handle_inner().await {
            tracing::error!("{}: ERROR: {}", &self.url, e);

            // Bump the failure count for the relay.
            self.dbrelay.failure_count += 1;
            if let Err(e) = DbRelay::update(self.dbrelay.clone()).await {
                tracing::error!("{}: ERROR bumping relay failure count: {}", &self.url, e);
            }
            // Update in globals too
            if let Some(mut dbrelay) = GLOBALS.all_relays.get_mut(&self.dbrelay.url) {
                dbrelay.failure_count += 1;
            }
        }

        tracing::info!("{}: minion exiting", self.url);
    }

    async fn handle_inner(&mut self) -> Result<(), Error> {
        tracing::trace!("{}: Minion handling started", &self.url); // minion will log when it connects

        // Connect to the relay
        let websocket_stream = {
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
            let request_nip11_future = reqwest::Client::builder()
                .timeout(std::time::Duration::new(30, 0))
                .redirect(reqwest::redirect::Policy::none())
                .gzip(true)
                .brotli(true)
                .deflate(true)
                .build()?
                .get(format!("{}", uri))
                .header("Accept", "application/nostr+json")
                .send();
            let response = request_nip11_future.await?;
            match Self::text_with_charset(response, "utf-8").await {
                Ok(text) => match serde_json::from_str::<RelayInformationDocument>(&text) {
                    Ok(nip11) => {
                        tracing::info!("{}: {}", &self.url, nip11);
                        self.nip11 = Some(nip11);
                    }
                    Err(e) => {
                        tracing::warn!(
                            "{}: Unable to parse response as NIP-11 ({}): {}",
                            &self.url,
                            e,
                            text
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!("{}: Unable to read response: {}", &self.url, e);
                }
            }

            let key: [u8; 16] = rand::random();

            let req = http::request::Request::builder().method("GET");

            let req = if GLOBALS.settings.read().set_user_agent {
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
                max_message_size: Some(1024 * 1024), // 1 MB
                max_frame_size: Some(1024 * 1024),   // 1 MB
                accept_unmasked_frames: false,       // default is false which is the standard
            };

            let (websocket_stream, _response) = tokio::time::timeout(
                std::time::Duration::new(15, 0),
                tokio_tungstenite::connect_async_with_config(req, Some(config)),
            )
            .await??;
            tracing::info!("{}: Connected", &self.url);

            websocket_stream
        };

        let (sink, stream) = websocket_stream.split();
        self.stream = Some(stream);
        self.sink = Some(sink);

        // Bump the success count for the relay
        {
            self.dbrelay.success_count += 1;
            self.dbrelay.last_connected_at = Some(Unixtime::now().unwrap().0 as u64);
            if let Err(e) = DbRelay::update(self.dbrelay.clone()).await {
                tracing::error!("{}: ERROR bumping relay success count: {}", &self.url, e);
            }
            // set in globals
            if let Some(mut dbrelay) = GLOBALS.all_relays.get_mut(&self.dbrelay.url) {
                dbrelay.last_connected_at = self.dbrelay.last_connected_at;
            }
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
                    // Log them and keep going
                    tracing::error!("{}: {}", &self.url, e);
                }
            }
        }

        // Close the connection
        let ws_sink = self.sink.as_mut().unwrap();
        if let Err(e) = ws_sink.send(WsMessage::Close(None)).await {
            tracing::error!("websocket close error: {}", e);
        }

        Ok(())
    }

    async fn loop_handler(&mut self) -> Result<(), Error> {
        let ws_stream = self.stream.as_mut().unwrap();
        let ws_sink = self.sink.as_mut().unwrap();

        let mut timer = tokio::time::interval(std::time::Duration::new(55, 0));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        timer.tick().await; // use up the first immediate tick.

        select! {
            _ = timer.tick() => {
                ws_sink.send(WsMessage::Ping(vec![0x1])).await?;
            },
            ws_message = ws_stream.next() => {
                let ws_message = match ws_message {
                    Some(m) => m,
                    None => {
                        // possibly connection reset
                        self.keepgoing = false;
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
                    self.handle_overlord_message(to_minion_message).await?;
                }
            },
        }

        Ok(())
    }

    pub async fn handle_overlord_message(&mut self, message: ToMinionMessage) -> Result<(), Error> {
        match message.payload {
            ToMinionPayload::FetchEvent(id) => {
                self.get_event(id).await?;
            }
            ToMinionPayload::PostEvent(event) => {
                let msg = ClientMessage::Event(event);
                let wire = serde_json::to_string(&msg)?;
                let ws_sink = self.sink.as_mut().unwrap();
                ws_sink.send(WsMessage::Text(wire)).await?;
                tracing::info!("Posted event to {}", &self.url);
            }
            ToMinionPayload::PullFollowing => {
                self.pull_following().await?;
            }
            ToMinionPayload::Shutdown => {
                tracing::info!("{}: Websocket listener shutting down", &self.url);
                self.keepgoing = false;
            }
            ToMinionPayload::SubscribeGeneralFeed(pubkeys) => {
                self.subscribe_general_feed(pubkeys).await?;
            }
            ToMinionPayload::SubscribeMentions => {
                self.subscribe_mentions().await?;
            }
            ToMinionPayload::SubscribeConfig => {
                self.subscribe_config().await?;
            }
            ToMinionPayload::SubscribePersonFeed(pubkeyhex) => {
                self.subscribe_person_feed(pubkeyhex).await?;
            }
            ToMinionPayload::SubscribeThreadFeed(main, parents) => {
                self.subscribe_thread_feed(main, parents).await?;
            }
            ToMinionPayload::TempSubscribeMetadata(pubkeyhexs) => {
                self.temp_subscribe_metadata(pubkeyhexs).await?;
            }
            ToMinionPayload::UnsubscribePersonFeed => {
                self.unsubscribe("person_feed").await?;
                // Close down if we aren't handling any more subscriptions
                if self.subscriptions.is_empty() {
                    self.keepgoing = false;
                }
            }
            ToMinionPayload::UnsubscribeThreadFeed => {
                self.unsubscribe("thread_feed").await?;
                // Close down if we aren't handling any more subscriptions
                if self.subscriptions.is_empty() {
                    self.keepgoing = false;
                }
            }
        }

        Ok(())
    }

    async fn tell_overlord_we_are_ready(&self) -> Result<(), Error> {
        self.to_overlord.send(ToOverlordMessage::MinionIsReady)?;
        Ok(())
    }

    // Subscribe to the user's followers on the relays they write to
    async fn subscribe_general_feed(
        &mut self,
        followed_pubkeys: Vec<PublicKeyHex>,
    ) -> Result<(), Error> {
        let mut filters: Vec<Filter> = Vec::new();
        let (overlap, feed_chunk) = {
            let settings = GLOBALS.settings.read().clone();
            (
                Duration::from_secs(settings.overlap),
                Duration::from_secs(settings.feed_chunk),
            )
        };

        tracing::debug!(
            "Following {} people at {}",
            followed_pubkeys.len(),
            &self.url
        );

        // Compute how far to look back
        let feed_since = {
            let now = Unixtime::now().unwrap();

            if self.subscriptions.has("general_feed") {
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

        // Allow all feed related event kinds
        let mut event_kinds = GLOBALS.settings.read().feed_related_event_kinds();
        // But exclude DMs in the general feed
        event_kinds.retain(|f| *f != EventKind::EncryptedDirectMessage);

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
                .map(|pk| pk.to_owned().into())
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
                .map(|pk| pk.into())
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
        } else {
            self.subscribe(filters, "general_feed").await?;

            if let Some(sub) = self.subscriptions.get_mut("general_feed") {
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
    async fn subscribe_mentions(&mut self) -> Result<(), Error> {
        let mut filters: Vec<Filter> = Vec::new();
        let (overlap, replies_chunk) = {
            let settings = GLOBALS.settings.read().clone();
            (
                Duration::from_secs(settings.overlap),
                Duration::from_secs(settings.replies_chunk),
            )
        };

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

        // Allow all feed related event kinds
        let event_kinds = GLOBALS.settings.read().feed_related_event_kinds();

        if let Some(pubkey) = GLOBALS.signer.public_key() {
            // Any mentions of me
            // (but not in peoples contact lists, for example)

            let pkh: PublicKeyHex = pubkey.into();

            filters.push(Filter {
                p: vec![pkh],
                kinds: event_kinds,
                since: Some(replies_since),
                ..Default::default()
            });
        }

        self.subscribe(filters, "mentions_feed").await?;

        if let Some(sub) = self.subscriptions.get_mut("mentions_feed") {
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
    async fn subscribe_config(&mut self) -> Result<(), Error> {
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

            self.subscribe(filters, "config_feed").await?;
        }

        Ok(())
    }

    // Subscribe to the posts a person generates on the relays they write to
    async fn subscribe_person_feed(&mut self, pubkey: PublicKeyHex) -> Result<(), Error> {
        // NOTE we do not unsubscribe to the general feed

        // Allow all feed related event kinds
        let mut event_kinds = GLOBALS.settings.read().feed_related_event_kinds();
        // Exclude DMs and reactions (we wouldn't see the post it reacted to) in person feed
        event_kinds
            .retain(|f| *f != EventKind::EncryptedDirectMessage && *f != EventKind::Reaction);

        let filters: Vec<Filter> = vec![Filter {
            authors: vec![pubkey.clone().into()],
            kinds: event_kinds,
            // No since, just a limit on quantity of posts
            limit: Some(25),
            ..Default::default()
        }];

        // let feed_chunk = GLOBALS.settings.read().await.feed_chunk;

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
        } else {
            self.subscribe(filters, "person_feed").await?;
        }

        Ok(())
    }

    async fn subscribe_thread_feed(
        &mut self,
        main: IdHex,
        vec_ids: Vec<IdHex>,
    ) -> Result<(), Error> {
        // NOTE we do not unsubscribe to the general feed

        let mut filters: Vec<Filter> = Vec::new();

        let enable_reactions = GLOBALS.settings.read().reactions;

        if !vec_ids.is_empty() {
            let idhp: Vec<IdHexPrefix> = vec_ids.iter().map(|id| id.to_owned().into()).collect();

            // Get ancestors we know of so far
            filters.push(Filter {
                ids: idhp,
                ..Default::default()
            });

            // Get reactions to ancestors, but not replies
            let mut kinds = vec![EventKind::EventDeletion];
            if enable_reactions {
                kinds.push(EventKind::Reaction);
            }
            filters.push(Filter {
                e: vec_ids,
                kinds,
                ..Default::default()
            });
        }

        // Allow all feed related event kinds
        let mut event_kinds = GLOBALS.settings.read().feed_related_event_kinds();
        // But exclude DMs
        event_kinds.retain(|f| *f != EventKind::EncryptedDirectMessage);

        filters.push(Filter {
            e: vec![main],
            kinds: event_kinds,
            ..Default::default()
        });

        self.subscribe(filters, "thread_feed").await?;

        Ok(())
    }

    // Create or replace the following subscription
    /*
    async fn upsert_following(&mut self, pubkeys: Vec<PublicKeyHex>) -> Result<(), Error> {
        let websocket_sink = self.sink.as_mut().unwrap();

        if pubkeys.is_empty() {
            if let Some(sub) = self.subscriptions.get("following") {
                // Close the subscription
                let wire = serde_json::to_string(&sub.close_message())?;
                websocket_sink.send(WsMessage::Text(wire.clone())).await?;

                // Remove the subscription from the map
                self.subscriptions.remove("following");
            }

            // Since pubkeys is empty, nothing to subscribe to.
            return Ok(());
        }

        // Compute how far to look back
        let (feed_since, special_since) = {
            // Get related settings
            let (overlap, feed_chunk) = {
                let settings = GLOBALS.settings.read().await.clone();
                (settings.overlap, settings.feed_chunk)
            };

            /*
            // Find the oldest 'last_fetched' among the 'person_relay' table.
            // Null values will come through as 0.
            let mut special_since: i64 =
                DbPersonRelay::fetch_oldest_last_fetched(&pubkeys, &self.url.0).await? as i64;
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
        let req_message = if self.subscriptions.has("following") {
            let sub = self.subscriptions.get_mut("following").unwrap();
            let vec: &mut Vec<Filter> = sub.get_mut();
            vec.clear();
            vec.push(feed_filter);
            vec.push(special_filter);
            sub.req_message()
        } else {
            self.subscriptions
                .add("following", vec![feed_filter, special_filter]);
            self.subscriptions.get("following").unwrap().req_message()
        };

        // Subscribe (or resubscribe) to the subscription
        let wire = serde_json::to_string(&req_message)?;
        websocket_sink.send(WsMessage::Text(wire.clone())).await?;

        tracing::trace!("{}: Sent {}", &self.url, &wire);

        Ok(())
    }
     */

    async fn get_event(&mut self, id: IdHex) -> Result<(), Error> {
        // create the filter
        let mut filter = Filter::new();
        filter.ids = vec![id.into()];

        tracing::trace!("{}: Event Filter: {} events", &self.url, filter.ids.len());

        // create a handle for ourselves
        let handle = format!("temp_events_{}", self.next_events_subscription_id);
        self.next_events_subscription_id += 1;

        // save the subscription
        let id = self.subscriptions.add(&handle, vec![filter]);
        tracing::debug!(
            "NEW SUBSCRIPTION on {} handle={}, id={}",
            &self.url,
            handle,
            &id
        );

        // get the request message
        let req_message = self.subscriptions.get(&handle).unwrap().req_message();

        // Subscribe on the relay
        let websocket_sink = self.sink.as_mut().unwrap();
        let wire = serde_json::to_string(&req_message)?;
        websocket_sink.send(WsMessage::Text(wire.clone())).await?;

        tracing::trace!("{}: Sent {}", &self.url, &wire);

        Ok(())
    }

    async fn temp_subscribe_metadata(
        &mut self,
        mut pubkeyhexs: Vec<PublicKeyHex>,
    ) -> Result<(), Error> {
        let pkhp: Vec<PublicKeyHexPrefix> = pubkeyhexs.drain(..).map(|pk| pk.into()).collect();

        tracing::trace!("Temporarily subscribing to metadata on {}", &self.url);

        let handle = "temp_subscribe_metadata".to_string();
        let filter = Filter {
            authors: pkhp,
            kinds: vec![EventKind::Metadata],
            // FIXME: we could probably get a since-last-fetched-their-metadata here.
            //        but relays should just return the lastest of these.
            ..Default::default()
        };
        self.subscribe(vec![filter], &handle).await
    }

    async fn pull_following(&mut self) -> Result<(), Error> {
        if let Some(pubkey) = GLOBALS.signer.public_key() {
            let pkh: PublicKeyHex = pubkey.into();
            let filter = Filter {
                authors: vec![pkh.into()],
                kinds: vec![EventKind::ContactList],
                ..Default::default()
            };
            self.subscribe(vec![filter], "following").await?;
        }
        Ok(())
    }

    async fn subscribe(&mut self, filters: Vec<Filter>, handle: &str) -> Result<(), Error> {
        if self.subscriptions.has(handle) {
            // Unsubscribe. will resubscribe under a new handle.
            self.unsubscribe(handle).await?;

            // Gratitously bump the EOSE as if the relay was finished, since it was
            // our fault the subscription is getting cut off.  This way we will pick up
            // where we left off instead of potentially loading a bunch of events
            // yet again.
            let now = Unixtime::now().unwrap();
            DbRelay::update_general_eose(self.dbrelay.url.clone(), now.0 as u64).await?;
        }
        let id = self.subscriptions.add(handle, filters);
        tracing::debug!(
            "NEW SUBSCRIPTION on {} handle={}, id={}",
            &self.url,
            handle,
            &id
        );
        let req_message = self.subscriptions.get(handle).unwrap().req_message();
        let wire = serde_json::to_string(&req_message)?;
        let websocket_sink = self.sink.as_mut().unwrap();
        tracing::trace!("{}: Sending {}", &self.url, &wire);
        websocket_sink.send(WsMessage::Text(wire.clone())).await?;
        Ok(())
    }

    async fn unsubscribe(&mut self, handle: &str) -> Result<(), Error> {
        if !self.subscriptions.has(handle) {
            return Ok(());
        }
        let close_message = self.subscriptions.get(handle).unwrap().close_message();
        let wire = serde_json::to_string(&close_message)?;
        let websocket_sink = self.sink.as_mut().unwrap();
        tracing::trace!("{}: Sending {}", &self.url, &wire);
        websocket_sink.send(WsMessage::Text(wire.clone())).await?;
        let id = self.subscriptions.remove(handle);
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
}
