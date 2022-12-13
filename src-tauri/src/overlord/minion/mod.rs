use crate::db::{DbPersonRelay, DbRelay};
use crate::{BusMessage, Error, GLOBALS, Settings};
use futures::{SinkExt, StreamExt};
use http::Uri;
use nostr_proto::{EventKind, Filters, PublicKeyHex, RelayInformationDocument, Unixtime, Url};
use std::collections::HashMap;
use tokio::select;
use tokio::net::TcpStream;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::{WebSocketStream, MaybeTlsStream};
use tungstenite::protocol::{Message as WsMessage, WebSocketConfig};


mod handle_bus;
mod handle_websocket;
mod subscription;
use subscription::Subscription;

pub struct Minion {
    url: Url,
    pubkeys: Vec<PublicKeyHex>,
    to_overlord: UnboundedSender<BusMessage>,
    from_overlord: Receiver<BusMessage>,
    settings: Settings,
    dbrelay: Option<DbRelay>,
    nip11: Option<RelayInformationDocument>,
    stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    subscriptions: HashMap<String, Subscription>,
}

impl Minion {
    pub fn new(url: Url, pubkeys: Vec<PublicKeyHex>) -> Minion {
        let to_overlord = GLOBALS.to_overlord.clone();
        let from_overlord = GLOBALS.to_minions.subscribe();

        Minion {
            url, pubkeys, to_overlord, from_overlord,
            settings: Default::default(),
            dbrelay: None,
            nip11: None,
            stream: None,
            subscriptions: HashMap::new(),
        }
    }
}

impl Minion {
    pub async fn handle(&mut self) {
        // Catch errors, Return nothing.
        if let Err(e) = self.handle_inner().await {
            log::error!("ERROR handling {}: {}", &self.url, e);
        }

        // Bump the failure count for the relay.
        if let Some(dbrelay) = &mut self.dbrelay {
            dbrelay.failure_count += 1;
            if let Err(e) = DbRelay::update(dbrelay.clone()).await {
                log::error!("ERROR bumping relay failure count {}: {}", &self.url, e);
            }
        }

        log::debug!("Minion exiting: {}", self.url);
    }

    async fn handle_inner(&mut self) -> Result<(), Error> {
        log::info!("Task started to handle relay at {}", &self.url);

        // Load settings
        self.settings.load().await?;

        // Connect to the relay
        let websocket_stream = {
            let uri: http::Uri = self.url.0.parse::<Uri>()?;
            let authority = uri.authority().ok_or(Error::UrlHasNoHostname)?.as_str();
            let host = authority
                .find('@')
                .map(|idx| authority.split_at(idx + 1).1)
                .unwrap_or_else(|| authority);
            if host.is_empty() {
                return Err(Error::UrlHasEmptyHostname);
            }

            // Read NIP-11 information
            if let Ok(response) = reqwest::Client::new()
                .get(&format!("https://{}", host))
                .header("Host", host)
                .header("Accept", "application/nostr+json")
                .send().await
            {
                match response.json::<RelayInformationDocument>().await
                {
                    Ok(nip11) => {
                        log::info!("{:?}", &nip11);
                        self.nip11 = Some(nip11);
                    },
                    Err(e) => {
                        log::error!("Unable to parse response as NIP-11 {}", e);
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
                .header("Sec-WebSocket-Key", base64::encode(&key))
                .uri(uri)
                .body(())?;

            let config: WebSocketConfig = WebSocketConfig {
                max_send_queue: None,
                max_message_size: Some(1024*1024*16), // their default is 64 MiB, I choose 16 MiB
                max_frame_size: Some(1024*1024*16), // their default is 16 MiB.
                accept_unmasked_frames: true, // default is false which is the standard
            };

            let (websocket_stream, _response) =
                tokio_tungstenite::connect_async_with_config(req, Some(config)).await?;
            log::info!("Connected to {}", &self.url);

            websocket_stream
        };

        self.stream = Some(websocket_stream);

        // Bump the success count for the relay
        {
            let maybe_dbrelay = DbRelay::fetch_one(&self.url).await?;
            if let Some(mut dbrelay) = maybe_dbrelay {
                dbrelay.success_count += 1;
                DbRelay::update(dbrelay.clone()).await?;
                self.dbrelay = Some(dbrelay);
            } else {
                log::error!("Could not load relay to update success count: {}", self.url);
            }
        }

        // Subscribe to the people we follow
        if self.pubkeys.len() > 0 {
            self.update_following_subscription().await?;
        }

        // Tell the overlord we are ready to receive commands
        self.tell_overlord_we_are_ready().await?;

        'relayloop:
        loop {
            match self.loop_handler().await {
                Ok(keepgoing) => {
                    if !keepgoing {
                        break 'relayloop;
                    }
                },
                Err(e) => {
                    // Log them and keep going
                    log::error!("{}", e);
                }
            }
        }

        Ok(())
    }

    async fn loop_handler(&mut self) -> Result<bool, Error> {
        let mut keepgoing: bool = true;

        let ws_stream = self.stream.as_mut().unwrap();

        select! {
            ws_message = ws_stream.next() => {
                let ws_message = match ws_message {
                    Some(m) => m,
                    None => return Ok(false), // probably connection reset
                }?;

                log::trace!("Handling message from {}", &self.url);
                match ws_message {
                    WsMessage::Text(t) => {
                        self.handle_nostr_message(t).await?;
                        // FIXME: some errors we should probably bail on.
                        // For now, try to continue.
                    },
                    WsMessage::Binary(_) => log::warn!("Unexpected binary message"),
                    WsMessage::Ping(x) => ws_stream.send(WsMessage::Pong(x)).await?,
                    WsMessage::Pong(_) => log::warn!("Unexpected pong message"),
                    WsMessage::Close(_) => keepgoing = false,
                    WsMessage::Frame(_) => log::warn!("Unexpected frame message"),
                }
            },
            bus_message = self.from_overlord.recv() => {
                let bus_message = match bus_message {
                    Ok(bm) => bm,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return Ok(false);
                    },
                    Err(e) => return Err(e.into())
                };
                if bus_message.target == self.url.0 {
                    self.handle_bus_message(bus_message).await?;
                } else if &*bus_message.target == "all" {
                    if &*bus_message.kind == "shutdown" {
                        log::info!("Websocket listener {} shutting down", &self.url);
                        keepgoing = false;
                    } else if &*bus_message.kind == "settings_changed" {
                        self.settings = serde_json::from_str(&bus_message.payload)?;
                    }
                }
            },
        }

        Ok(keepgoing)
    }

    async fn tell_overlord_we_are_ready(
        &self,
    ) -> Result<(), Error> {
        self.to_overlord.send(BusMessage {
            relay_url: Some(self.url.0.clone()),
            target: "overlord".to_string(),
            kind: "minion_is_ready".to_string(),
            payload: "".to_owned(),
        })?;

        Ok(())
    }

    async fn update_following_subscription(&mut self) -> Result<(), Error> {

        let websocket_stream = self.stream.as_mut().unwrap();

        if self.pubkeys.len() == 0 {
            if let Some(sub) = self.subscriptions.get("following") {
                // Close the subscription
                let wire = serde_json::to_string(&sub.close_message())?;
                websocket_stream.send(WsMessage::Text(wire.clone())).await?;

                // Remove the subscription from the map
                self.subscriptions.remove("following");
            }

            // Since pubkeys is empty, nothing to subscribe to.
            return Ok(());
        }

        // Compute how far to look back
        let (feed_since, special_since) = {
            // Find the oldest 'last_fetched' among the 'person_relay' table.
            // Null values will come through as 0.
            let mut special_since: i64 = DbPersonRelay::fetch_oldest_last_fetched(
                &self.pubkeys,
                &self.url.0
            ).await? as i64;

            // Subtract overlap to avoid gaps due to clock sync and event
            // propogation delay
            special_since -= self.settings.overlap as i64;

            // For feed related events, don't look back more than one feed_chunk ago
            let one_feedchunk_ago = Unixtime::now().unwrap().0 - self.settings.feed_chunk as i64;
            let feed_since = special_since.max(one_feedchunk_ago);

            (Unixtime(feed_since), Unixtime(special_since))
        };

        // Create the author filter
        let mut feed_filter: Filters = Filters::new();
        for pk in self.pubkeys.iter() {
            feed_filter.add_author(&pk, None);
        }
        feed_filter.add_event_kind(EventKind::TextNote);
        feed_filter.add_event_kind(EventKind::Reaction);
        feed_filter.add_event_kind(EventKind::EventDeletion);
        feed_filter.since = Some(feed_since);
        log::debug!(
            "Feed Filter {}: {}",
            &self.url,
            serde_json::to_string(&feed_filter)?
        );

        // Create the lookback filter
        let mut special_filter: Filters = Filters::new();
        for pk in self.pubkeys.iter() {
            special_filter.add_author(&pk, None);
        }
        special_filter.add_event_kind(EventKind::Metadata);
        //special_filter.add_event_kind(EventKind::RecommendRelay);
        //special_filter.add_event_kind(EventKind::ContactList);
        special_filter.since = Some(special_since);
        log::debug!(
            "Special Filter {}: {}",
            &self.url,
            serde_json::to_string(&special_filter)?
        );

        // Get the subscription
        let sub = self.subscriptions.entry("following".to_string()).or_insert(
            Subscription::new("following".to_string())
        );

        // Write our filters into it
        {
            let vec: &mut Vec<Filters> = sub.get_mut();
            vec.clear();
            vec.push(feed_filter);
            vec.push(special_filter);
        }

        // Subscribe (or resubscribe) to the subscription
        let wire = serde_json::to_string(&sub.req_message())?;
        websocket_stream.send(WsMessage::Text(wire.clone())).await?;

        log::trace!("Sent {}", &wire);

        Ok(())
    }
}
