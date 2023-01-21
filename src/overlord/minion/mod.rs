mod handle_websocket;
mod subscription;

use crate::comms::{ToMinionMessage, ToMinionPayload, ToOverlordMessage};
use crate::db::DbRelay;
use crate::error::Error;
use crate::globals::GLOBALS;
use futures::{SinkExt, StreamExt};
use futures_util::stream::{SplitSink, SplitStream};
use http::Uri;
use nostr_types::{
    ClientMessage, EventKind, Filter, IdHex, PublicKeyHex, RelayInformationDocument, Unixtime, Url,
};
use std::time::Duration;
use subscription::Subscriptions;
use tokio::net::TcpStream;
use tokio::select;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::protocol::{Message as WsMessage, WebSocketConfig};

pub struct Minion {
    url: Url,
    to_overlord: UnboundedSender<ToOverlordMessage>,
    from_overlord: Receiver<ToMinionMessage>,
    dbrelay: DbRelay,
    nip11: Option<RelayInformationDocument>,
    stream: Option<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>,
    sink: Option<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, WsMessage>>,
    subscriptions: Subscriptions,
    next_events_subscription_id: u32,
}

impl Minion {
    pub async fn new(url: Url) -> Result<Minion, Error> {
        if !url.is_valid_relay_url() {
            return Err(Error::InvalidUrl(url.inner().to_owned()));
        }

        let to_overlord = GLOBALS.to_overlord.clone();
        let from_overlord = GLOBALS.to_minions.subscribe();
        let dbrelay = match DbRelay::fetch_one(&url).await? {
            Some(dbrelay) => dbrelay,
            None => {
                let dbrelay = DbRelay::new(url.inner().to_owned())?;
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
        }

        tracing::info!("{}: minion exiting", self.url);
    }

    async fn handle_inner(&mut self) -> Result<(), Error> {
        tracing::trace!("{}: Minion handling started", &self.url); // minion will log when it connects

        // Connect to the relay
        let websocket_stream = {
            let uri: http::Uri = self.url.inner().parse::<Uri>()?;
            let authority = uri.authority().ok_or(Error::UrlHasNoHostname)?.as_str();
            let host = authority
                .find('@')
                .map(|idx| authority.split_at(idx + 1).1)
                .unwrap_or_else(|| authority);
            if host.is_empty() {
                return Err(Error::UrlHasEmptyHostname);
            }

            let request_nip11_future = reqwest::Client::new()
                .get(format!("https://{}", host))
                .header("Host", host)
                .header("Accept", "application/nostr+json")
                .send();

            // Read NIP-11 information
            if let Ok(response) =
                tokio::time::timeout(std::time::Duration::new(15, 0), request_nip11_future).await?
            {
                match response.json::<RelayInformationDocument>().await {
                    Ok(nip11) => {
                        tracing::info!("{}: {}", &self.url, nip11);
                        self.nip11 = Some(nip11);
                    }
                    Err(e) => {
                        tracing::warn!("{}: Unable to parse response as NIP-11: {}", &self.url, e);
                    }
                }
            }

            let key: [u8; 16] = rand::random();

            let req = http::request::Request::builder()
                .method("GET")
                .header("Host", host)
                .header("Connection", "Upgrade")
                .header("Upgrade", "websocket")
                .header("Sec-WebSocket-Version", "13")
                .header("Sec-WebSocket-Key", base64::encode(key))
                .uri(uri)
                .body(())?;

            let config: WebSocketConfig = WebSocketConfig {
                max_send_queue: None,
                max_message_size: Some(1024 * 1024 * 16), // their default is 64 MiB, I choose 16 MiB
                max_frame_size: Some(1024 * 1024 * 16),   // their default is 16 MiB.
                accept_unmasked_frames: true,             // default is false which is the standard
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
            let now = Unixtime::now().unwrap().0 as u64;
            DbRelay::update_success(self.dbrelay.url.clone(), now).await?;
        }

        // Tell the overlord we are ready to receive commands
        self.tell_overlord_we_are_ready().await?;

        'relayloop: loop {
            match self.loop_handler().await {
                Ok(keepgoing) => {
                    if !keepgoing {
                        break 'relayloop;
                    }
                }
                Err(e) => {
                    // Log them and keep going
                    tracing::error!("{}: {}", &self.url, e);
                }
            }
        }

        Ok(())
    }

    async fn loop_handler(&mut self) -> Result<bool, Error> {
        let mut keepgoing: bool = true;

        let ws_stream = self.stream.as_mut().unwrap();
        let ws_sink = self.sink.as_mut().unwrap();

        let mut timer = tokio::time::interval(std::time::Duration::new(55, 0));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        timer.tick().await; // use up the first immediate tick.

        select! {
            _ = timer.tick() => {
                ws_sink.send(WsMessage::Ping(vec![])).await?;
            },
            ws_message = ws_stream.next() => {
                let ws_message = match ws_message {
                    Some(m) => m,
                    None => return Ok(false), // probably connection reset
                }?;

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
                    WsMessage::Ping(x) => ws_sink.send(WsMessage::Pong(x)).await?,
                    WsMessage::Pong(_) => { }, // we just ignore pongs
                    WsMessage::Close(_) => keepgoing = false,
                    WsMessage::Frame(_) => tracing::warn!("{}: Unexpected frame message", &self.url),
                }
            },
            to_minion_message = self.from_overlord.recv() => {
                let to_minion_message = match to_minion_message {
                    Ok(m) => m,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return Ok(false);
                    },
                    Err(e) => return Err(e.into())
                };
                #[allow(clippy::collapsible_if)]
                if to_minion_message.target == self.url.inner() || to_minion_message.target == "all" {
                    keepgoing = self.handle_message(to_minion_message).await?;
                }
            },
        }

        Ok(keepgoing)
    }

    pub async fn handle_message(&mut self, message: ToMinionMessage) -> Result<bool, Error> {
        match message.payload {
            ToMinionPayload::FetchEvents(vec) => {
                self.get_events(vec).await?;
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
                return Ok(false);
            }
            ToMinionPayload::SubscribeGeneralFeed => {
                self.subscribe_general_feed().await?;
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
            ToMinionPayload::UnsubscribeThreadFeed => {
                self.unsubscribe_thread_feed().await?;
            }
        }
        Ok(true)
    }

    async fn tell_overlord_we_are_ready(&self) -> Result<(), Error> {
        self.to_overlord.send(ToOverlordMessage::MinionIsReady)?;
        Ok(())
    }

    async fn subscribe_general_feed(&mut self) -> Result<(), Error> {
        // NOTE if the general feed is already subscribed we shoudn't do anything
        // but we may need to update the subscription

        let mut filters: Vec<Filter> = Vec::new();
        let (overlap, feed_chunk, replies_chunk) = {
            let settings = GLOBALS.settings.read().await.clone();
            (
                Duration::from_secs(settings.overlap),
                Duration::from_secs(settings.feed_chunk),
                Duration::from_secs(settings.replies_chunk),
            )
        };

        let followed_pubkeys = GLOBALS.people.get_followed_pubkeys();
        tracing::debug!(
            "Following {} people at {}",
            followed_pubkeys.len(),
            &self.url
        );

        // Compute how far to look back
        let (feed_since, replies_since) = {
            // Start with where we left off, the time we last got something from
            // this relay.
            let mut replies_since: Unixtime = match self.dbrelay.last_general_eose_at {
                Some(u) => Unixtime(u as i64),
                None => Unixtime(0),
            };

            // Subtract overlap to avoid gaps due to clock sync and event
            // propogation delay
            replies_since = replies_since - overlap;

            // Some relays don't like dates before 1970.  Hell, we don't need anything before 2020:
            if replies_since.0 < 1577836800 {
                replies_since.0 = 1577836800;
            }

            let one_replieschunk_ago = Unixtime::now().unwrap() - replies_chunk;
            let replies_since = replies_since.max(one_replieschunk_ago);

            let one_feedchunk_ago = Unixtime::now().unwrap() - feed_chunk;
            let feed_since = replies_since.max(one_feedchunk_ago);

            (feed_since, replies_since)
        };

        let enable_reactions = GLOBALS.settings.read().await.reactions;

        if let Some(pubkey) = GLOBALS.signer.read().await.public_key() {
            let mut kinds = vec![
                EventKind::TextNote,
                EventKind::Repost,
                EventKind::EventDeletion,
            ];
            if enable_reactions {
                kinds.push(EventKind::Reaction);
            }

            // feed related by me
            filters.push(Filter {
                authors: vec![pubkey.into()],
                kinds,
                since: Some(feed_since),
                ..Default::default()
            });

            // Any mentions of me
            // (but not in peoples contact lists, for example)
            let mut kinds = vec![EventKind::TextNote, EventKind::Repost];
            if enable_reactions {
                kinds.push(EventKind::Reaction);
            }
            filters.push(Filter {
                p: vec![pubkey.into()],
                kinds: vec![EventKind::TextNote, EventKind::Repost],
                since: Some(replies_since),
                ..Default::default()
            });

            // my metadata
            // FIXME TBD
            /*
            filters.push(Filter {
                authors: vec![pubkey],
                kinds: vec![EventKind::Metadata, EventKind::RecommendRelay, EventKind::ContactList, EventKind::RelaysList],
                since: // last we last checked
                .. Default::default()
            });
             */
        }

        if !followed_pubkeys.is_empty() {
            let mut kinds = vec![
                EventKind::TextNote,
                EventKind::Repost,
                EventKind::EventDeletion,
            ];
            if enable_reactions {
                kinds.push(EventKind::Reaction);
            }
            // feed related by people followed
            filters.push(Filter {
                authors: followed_pubkeys.clone(),
                kinds,
                since: Some(feed_since),
                ..Default::default()
            });

            // metadata by people followed
            // FIXME TBD
            /*
            filters.push(Filter {
                authors: pubkeys.clone(),
                kinds: vec![EventKind::Metadata, EventKind::RecommendRelay, EventKind::ContactList, EventKind::RelaysList],
                since: // last we last checked
                .. Default::default()
            });
            */
        }

        // reactions to posts by me
        // FIXME TBD

        // reactions to posts by people followed
        // FIXME TBD

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

    async fn subscribe_person_feed(&mut self, pubkey: PublicKeyHex) -> Result<(), Error> {
        // NOTE we do not unsubscribe to the general feed

        let filters: Vec<Filter> = vec![Filter {
            authors: vec![pubkey.clone()],
            kinds: vec![EventKind::TextNote, EventKind::EventDeletion],
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

        let enable_reactions = GLOBALS.settings.read().await.reactions;

        if !vec_ids.is_empty() {
            // Get ancestors we know of so far
            filters.push(Filter {
                ids: vec_ids.clone(),
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

        // Get replies to main event
        let mut kinds = vec![
            EventKind::TextNote,
            EventKind::Repost,
            EventKind::EventDeletion,
        ];
        if enable_reactions {
            kinds.push(EventKind::Reaction);
        }
        filters.push(Filter {
            e: vec![main],
            kinds,
            ..Default::default()
        });

        self.subscribe(filters, "thread_feed").await?;

        Ok(())
    }

    async fn unsubscribe_thread_feed(&mut self) -> Result<(), Error> {
        self.unsubscribe("thread_feed").await?;
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
            // propogation delay
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

    async fn get_events(&mut self, ids: Vec<IdHex>) -> Result<(), Error> {
        if ids.is_empty() {
            return Ok(());
        }

        // create the filter
        let mut filter = Filter::new();
        filter.ids = ids;

        tracing::trace!("{}: Event Filter: {} events", &self.url, filter.ids.len());

        // create a handle for ourselves
        let handle = format!("temp_events_{}", self.next_events_subscription_id);
        self.next_events_subscription_id += 1;

        // save the subscription
        self.subscriptions.add(&handle, vec![filter]);

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
        pubkeyhexs: Vec<PublicKeyHex>,
    ) -> Result<(), Error> {
        let handle = "temp_subscribe_metadata".to_string();
        let filter = Filter {
            authors: pubkeyhexs,
            kinds: vec![EventKind::Metadata],
            // FIXME: we could probably get a since-last-fetched-their-metadata here.
            //        but relays should just return the lastest of these.
            ..Default::default()
        };
        self.subscribe(vec![filter], &handle).await
    }

    async fn pull_following(&mut self) -> Result<(), Error> {
        if let Some(pubkey) = GLOBALS.signer.read().await.public_key() {
            let filter = Filter {
                authors: vec![pubkey.into()],
                kinds: vec![EventKind::ContactList],
                ..Default::default()
            };
            self.subscribe(vec![filter], "following").await?;
        }
        Ok(())
    }

    #[allow(dead_code)]
    async fn subscribe(&mut self, filters: Vec<Filter>, handle: &str) -> Result<(), Error> {
        let req_message = if self.subscriptions.has(handle) {
            let sub = self.subscriptions.get_mut(handle).unwrap();
            *sub.get_mut() = filters;
            sub.req_message()
        } else {
            self.subscriptions.add(handle, filters);
            self.subscriptions.get(handle).unwrap().req_message()
        };
        let wire = serde_json::to_string(&req_message)?;
        let websocket_sink = self.sink.as_mut().unwrap();
        websocket_sink.send(WsMessage::Text(wire.clone())).await?;
        tracing::trace!("{}: Sent {}", &self.url, &wire);
        Ok(())
    }

    #[allow(dead_code)]
    async fn unsubscribe(&mut self, handle: &str) -> Result<(), Error> {
        if !self.subscriptions.has(handle) {
            return Ok(());
        }
        let close_message = self.subscriptions.get(handle).unwrap().close_message();
        let wire = serde_json::to_string(&close_message)?;
        let websocket_sink = self.sink.as_mut().unwrap();
        websocket_sink.send(WsMessage::Text(wire.clone())).await?;
        tracing::trace!("{}: Sent {}", &self.url, &wire);
        self.subscriptions.remove(handle);
        Ok(())
    }
}
