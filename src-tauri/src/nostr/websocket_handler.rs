use crate::db::{DbEvent, DbEventSeen, DbEventTag, DbPerson};
use crate::{BusMessage, Error, GLOBALS};
use futures::{SinkExt, StreamExt};
use nostr_proto::{
    ClientMessage, Event, EventKind, Filters, Metadata, RelayMessage,
    SubscriptionId, Unixtime, Url,
};
use tokio::select;
use tokio::sync::broadcast::Sender;
use tungstenite::protocol::Message as WsMessage;

pub struct WebsocketHandler {
    url: Url,
    filters: Filters // FIXME, get these via a bus message, as they change over time
}

impl WebsocketHandler {
    pub fn new(url: Url, filters: Filters) -> WebsocketHandler {
        WebsocketHandler { url, filters }
    }
}

impl WebsocketHandler {
    pub async fn handle(&self) {
        // Catch errors, Return nothing.
        if let Err(e) = self.handle_inner().await {
            log::error!("ERROR handling {}: {}", &self.url, e);
        }

        // Should we signal that we are exiting?
    }

    async fn handle_inner(&self) -> Result<(), Error> {
        log::info!("Task started to handle relay at {}", &self.url);

        log::debug!(
            "Filter for {}: {}",
            &self.url,
            serde_json::to_string(&self.filters)?
        );

        // Get the broadcast channel and subscribe to it
        let tx = GLOBALS.bus.clone();
        let mut rx = tx.subscribe();

        // Connect to the relay
        let (websocket_stream, _response) = tokio_tungstenite::connect_async(&self.url.0).await?;
        log::info!("Connected to {}", &self.url);

        let (mut write, mut read) = websocket_stream.split();

        // Subscribe to our filters
        // FIXME, get filters in response to an appropriate bus message
        let message = ClientMessage::Req(
            SubscriptionId(format!("gossip-main-{}", textnonce::TextNonce::new())),
            vec![self.filters.clone()],
        );
        let wire = serde_json::to_string(&message)?;
        write.send(WsMessage::Text(wire.clone())).await?;
        //log::debug!("Sent {}", &wire);

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
                    log::debug!("Handling message from {}", &self.url);
                    match ws_message {
                        WsMessage::Text(t) => {
                            if let Err(e) = self.handle_nostr_message(tx.clone(), t).await {
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
                bus_message = rx.recv() => {
                    if let Err(e) = bus_message {
                        log::error!("{}", e);
                        continue 'relayloop;
                    }
                    let bus_message = bus_message.unwrap();
                    if bus_message.target == self.url.0 {
                        log::warn!("Websocket task got message, unimpmented: {}",
                                   bus_message.payload);
                    } else if &*bus_message.target == "all" && &*bus_message.kind == "shutdown" {
                        log::info!("Websocket listener {} shutting down", &self.url);
                        break 'relayloop;
                    }
                },
            }
        }

        Ok(())
    }

    async fn handle_nostr_message(
        &self,
        tx: Sender<BusMessage>,
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
                    self.process_event(&tx, *event).await?;
                }
            }
            RelayMessage::Notice(msg) => {
                log::info!("NOTICE: {} {}", &self.url, msg);
            }
            RelayMessage::Eose(subid) => {
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

    async fn send_javascript_pushfeedevents(
        &self,
        tx: &Sender<BusMessage>,
        events: Vec<Event>
    ) -> Result<(), Error> {
        if let Err(e) = tx.send(BusMessage {
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
        tx: &Sender<BusMessage>,
        people: Vec<DbPerson>
    ) -> Result<(), Error> {
        if let Err(e) = tx.send(BusMessage {
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
        tx: &Sender<BusMessage>,
        event: Event
    ) -> Result<(), Error> {

        match event.kind {
            EventKind::Metadata => {
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
                    self.send_javascript_setpeople(&tx, vec![person]).await?;
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
                    self.send_javascript_setpeople(&tx, vec![person]).await?;
                }
            },
            EventKind::TextNote => {
                // Javascript needs to render this event on the feed:
                self.send_javascript_pushfeedevents(&tx, vec![event]).await?;
            },
            EventKind::RecommendRelay => {
                // TBD
            },
            EventKind::ContactList => {
                // TBD
            },
            EventKind::EventDeletion => {
                // TBD
            },
            EventKind::Reaction => {
                // TBD
            },
            _ => { }
        }

        Ok(())
    }
}

