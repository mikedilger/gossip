use crate::db::{DbEvent, DbEventSeen, DbEventTag, DbPerson, DbPersonRelay};
use crate::{BusMessage, Error, GLOBALS, Settings};
use futures::{SinkExt, StreamExt};
use nostr_proto::{
    ClientMessage, Event, EventKind, Filters, Metadata, PublicKeyHex,
    RelayMessage, SubscriptionId, Unixtime, Url,
};
use tokio::select;
use tokio::sync::broadcast::{Sender, Receiver};
use tungstenite::protocol::Message as WsMessage;

pub struct Minion {
    url: Url,
    pubkeys: Vec<PublicKeyHex>,
    bus_tx: Sender<BusMessage>,
    bus_rx: Receiver<BusMessage>,
    settings: Settings,
}

impl Minion {
    pub fn new(url: Url, pubkeys: Vec<PublicKeyHex>) -> Minion {
        // Get the broadcast channel and subscribe to it
        let bus_tx = GLOBALS.bus.clone();
        let bus_rx = bus_tx.subscribe();

        Minion {
            url, pubkeys, bus_tx, bus_rx,
            settings: Default::default()
        }
    }
}

impl Minion {
    pub async fn handle(&mut self) {
        // Catch errors, Return nothing.
        if let Err(e) = self.handle_inner().await {
            log::error!("ERROR handling {}: {}", &self.url, e);
        }

        // Should we signal that we are exiting?
    }

    async fn handle_inner(&mut self) -> Result<(), Error> {
        log::info!("Task started to handle relay at {}", &self.url);

        // Load settings
        self.settings.load().await?;

        // Compute how far to look back for events
        let since = {
            // Find the oldest 'last_fetched' among the 'person_relay' table
            let mut since: u64 = DbPersonRelay::fetch_oldest_last_fetched(
                &self.pubkeys,
                &self.url.0
            ).await?;

            // Subtract overlap to avoid gaps due to clock sync and event
            // propogation delay
            since -= self.settings.overlap;

            // But don't go back more than one feed_chunk ago
            let one_feedchunk_ago = Unixtime::now().unwrap().0 - self.settings.feed_chunk as i64;

            since = since.max(one_feedchunk_ago as u64);

            log::debug!("Looking back to unixtime {}", since);

            Unixtime(since as i64)
        };

        // Create the author filter
        let mut author_filter: Filters = Filters::new();
        for pk in self.pubkeys.iter() {
            author_filter.add_author(&pk, None);
        }
        author_filter.add_event_kind(EventKind::TextNote);
        author_filter.add_event_kind(EventKind::Reaction);
        author_filter.since = Some(since);
        log::debug!(
            "Author Filter {}: {}",
            &self.url,
            serde_json::to_string(&author_filter)?
        );

        // Create the lookback filter
        let mut lookback_filter: Filters = Filters::new();
        for pk in self.pubkeys.iter() {
            lookback_filter.add_author(&pk, None);
        }
        lookback_filter.add_event_kind(EventKind::Metadata);
        //lookback_filter.add_event_kind(EventKind::RecommendRelay);
        //lookback_filter.add_event_kind(EventKind::ContactList);
        //lookback_filter.add_event_kind(EventKind::EventDeletion);
        log::debug!(
            "Lookback Filter {}: {}",
            &self.url,
            serde_json::to_string(&lookback_filter)?
        );

        // Connect to the relay
        let (websocket_stream, _response) = tokio_tungstenite::connect_async(&self.url.0).await?;
        log::info!("Connected to {}", &self.url);

        let (mut write, mut read) = websocket_stream.split();

        // Subscribe to our filters
        // FIXME, get filters in response to an appropriate bus message
        let message = ClientMessage::Req(
            SubscriptionId(format!("gossip-main-{}", textnonce::TextNonce::new())),
            vec![author_filter, lookback_filter],
        );
        let wire = serde_json::to_string(&message)?;
        write.send(WsMessage::Text(wire.clone())).await?;
        //log::debug!("Sent {}", &wire);

        // Tell the overlord we are ready to receive commands
        self.tell_overlord_we_are_ready().await?;

        'relayloop: loop {
            select! {
                ws_message = read.next() => {
                    let ws_message = match ws_message.unwrap() {
                        Ok(wsm) => wsm,
                        Err(e) => {
                            log::error!("{}", e);
                            // We probably cannot continue the websocket
                            break 'relayloop;
                        }
                    };
                    log::trace!("Handling message from {}", &self.url);
                    match ws_message {
                        WsMessage::Text(t) => {
                            if let Err(e) = self.handle_nostr_message(t).await {
                                log::error!("Error on {}: {}", &self.url, e);
                                // FIXME: some errors we should probably bail on.
                                // For now, try to continue.
                            }
                        },
                        WsMessage::Binary(_) => log::warn!("Unexpected binary message"),
                        WsMessage::Ping(x) => write.send(WsMessage::Pong(x)).await?,
                        WsMessage::Pong(_) => log::warn!("Unexpected pong message"),
                        WsMessage::Close(_) => break 'relayloop,
                        WsMessage::Frame(_) => log::warn!("Unexpected frame message"),
                    }
                },
                bus_message = self.bus_rx.recv() => {
                    if let Err(e) = bus_message {
                        log::error!("{}", e);
                        continue 'relayloop;
                    }
                    let bus_message = bus_message.unwrap();
                    if bus_message.target == self.url.0 {
                        log::warn!("Websocket task got message, unimpmented: {}",
                                   bus_message.payload);
                    } else if &*bus_message.target == "all" {
                        if &*bus_message.kind == "shutdown" {
                            log::info!("Websocket listener {} shutting down", &self.url);
                            break 'relayloop;
                        } else if &*bus_message.kind == "settings_changed" {
                            match serde_json::from_str(&bus_message.payload) {
                                Ok(s) => self.settings=s,
                                Err(e) => log::error!("minion unable to update settings: {}", e),
                            }
                        }
                    }
                },
            }
        }

        Ok(())
    }

    async fn handle_nostr_message(
        &self,
        ws_message: String
    ) -> Result<(), Error> {

        // TODO: pull out the raw event without any deserialization to be sure we don't mangle
        //       it.

        let relay_message: RelayMessage = serde_json::from_str(&ws_message)?;

        let mut maxtime = Unixtime::now()?;
        maxtime.0 += 60 * 15; // 15 minutes into the future

        match relay_message {
            RelayMessage::Event(_subid, event) => {
                if let Err(e) = event.verify(Some(maxtime)) {
                    log::error!("VERIFY ERROR: {}, {}", e, serde_json::to_string(&event)?)
                } else {
                    self.save_event_in_database(&event).await?;
                    self.process_event(*event).await?;
                }
            }
            RelayMessage::Notice(msg) => {
                log::info!("NOTICE: {} {}", &self.url, msg);
            }
            RelayMessage::Eose(subid) => {
                // We should update last_fetched
                let now = Unixtime::now().unwrap().0 as u64;
                DbPersonRelay::update_last_fetched(self.url.0.clone(), now).await?;

                // These don't have to be processed.
                log::info!("EOSE: {} {:?}", &self.url, subid);
            }
            RelayMessage::Ok(id, ok, ok_message) => {
                // These don't have to be processed.
                log::info!("OK: {} {:?} {} {}", &self.url, id, ok, ok_message);
            }
        }

        Ok(())
    }

    async fn save_event_in_database(
        &self,
        event: &Event
    ) -> Result<(), Error> {
        let db_event = DbEvent {
            id: event.id.into(),
            raw: serde_json::to_string(&event)?, // TODO: this is reserialized.
            pubkey: event.pubkey.into(),
            created_at: event.created_at.0,
            kind: event.kind.into(),
            content: event.content.clone(),
            ots: event.ots.clone()
        };
        DbEvent::insert(db_event).await?;

        let mut seq = 0;
        for tag in event.tags.iter() {
            // convert to vec of strings
            let v: Vec<String> = serde_json::from_str(&serde_json::to_string(&tag)?)?;

            let db_event_tag = DbEventTag {
                event: event.id.as_hex_string(),
                seq: seq,
                label: v.get(0).cloned(),
                field0: v.get(1).cloned(),
                field1: v.get(2).cloned(),
                field2: v.get(3).cloned(),
                field3: v.get(4).cloned(),
            };
            DbEventTag::insert(db_event_tag).await?;
            seq += 1;
        }

        let db_event_seen = DbEventSeen {
            event: event.id.as_hex_string(),
            relay: self.url.0.clone(),
            when_seen: Unixtime::now()?.0 as u64
        };
        DbEventSeen::replace(db_event_seen).await?;

        Ok(())
    }

    async fn tell_overlord_we_are_ready(
        &self,
    ) -> Result<(), Error> {
        if let Err(e) = self.bus_tx.send(BusMessage {
            target: "overlord".to_string(),
            source: self.url.0.clone(),
            kind: "minion_is_ready".to_string(),
            payload: "".to_owned(),
        }) {
            log::error!("Unable to tell the overlord we are ready: {}", e);
        }

        Ok(())
    }

    async fn send_javascript_pushfeedevents(
        &self,
        events: Vec<Event>
    ) -> Result<(), Error> {
        if let Err(e) = self.bus_tx.send(BusMessage {
            target: "to_javascript".to_string(),
            source: self.url.0.clone(),
            kind: "pushfeedevents".to_string(),
            payload: serde_json::to_string(&events)?,
        }) {
            log::error!("Unable to send pushfeedevents to javascript: {}", e);
        }

        Ok(())
    }


    async fn send_javascript_setpeople(
        &self,
        people: Vec<DbPerson>
    ) -> Result<(), Error> {
        if let Err(e) = self.bus_tx.send(BusMessage {
            target: "to_javascript".to_string(),
            source: self.url.0.clone(),
            kind: "setpeople".to_string(),
            payload: serde_json::to_string(&people)?,
        }) {
            log::error!("Unable to send setpeople to javascript: {}", e);
        }

        Ok(())
    }

    async fn process_event(
        &self,
        event: Event
    ) -> Result<(), Error> {

        match event.kind {
            EventKind::Metadata => {
                log::debug!("Event(metadata) from {}", &self.url);
                let created_at: u64 = event.created_at.0 as u64;
                let metadata: Metadata = serde_json::from_str(&event.content)?;
                if let Some(mut person) = DbPerson::fetch_one(event.pubkey.into()).await? {
                    person.name = Some(metadata.name);
                    person.about = metadata.about;
                    person.picture = metadata.picture;
                    if person.dns_id != metadata.nip05 {
                        person.dns_id = metadata.nip05;
                        person.dns_id_valid = 0; // changed so starts invalid
                        person.dns_id_last_checked = match person.dns_id_last_checked {
                            None => Some(created_at),
                            Some(lc) => Some(created_at.max(lc)),
                        }
                    }
                    DbPerson::update(person.clone()).await?;
                    self.send_javascript_setpeople(vec![person]).await?;
                } else {
                    let person = DbPerson {
                        pubkey: event.pubkey.into(),
                        name: Some(metadata.name),
                        about: metadata.about,
                        picture: metadata.picture,
                        dns_id: metadata.nip05,
                        dns_id_valid: 0, // new so starts invalid
                        dns_id_last_checked: Some(created_at),
                        followed: 0
                    };
                    DbPerson::insert(person.clone()).await?;
                    self.send_javascript_setpeople(vec![person]).await?;
                }
            },
            EventKind::TextNote => {
                log::debug!("Event(textnote) from {}", &self.url);
                // Javascript needs to render this event on the feed:
                self.send_javascript_pushfeedevents(vec![event]).await?;
            },
            EventKind::RecommendRelay => {
                log::debug!("Event(recommend_relay) from {} [IGNORED]", &self.url);
                // TBD
            },
            EventKind::ContactList => {
                log::debug!("Event(contact_list) from {} [IGNORED]", &self.url);
                // TBD
            },
            EventKind::EventDeletion => {
                log::debug!("Event(deletion) from {} [IGNORED]", &self.url);
                // TBD
            },
            EventKind::Reaction => {
                log::debug!("Event(reaction) from {} [IGNORED]", &self.url);
                // TBD
            },
            _ => { }
        }

        Ok(())
    }
}
