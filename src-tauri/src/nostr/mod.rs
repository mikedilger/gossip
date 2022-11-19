use crate::db::{DbPerson, DbPersonRelay};
use crate::{BusMessage, Error, GLOBALS};
use futures::{SinkExt, StreamExt};
use nostr_proto::{
    ClientMessage, EventKind, Filters, PublicKey, RelayMessage, SubscriptionId, Unixtime, Url,
};
use std::collections::HashMap;
use tokio::select;
use tokio::sync::broadcast::Sender;
use tungstenite::protocol::Message as WsMessage;

/// This function computes which relays we need to follow and what filters
/// they should have, only for startup, based on what is in the database.
pub async fn load_initial_relay_filters() -> Result<HashMap<Url, Filters>, Error> {
    let mut hashmap: HashMap<Url, Filters> = HashMap::new();

    // Load all the people we are following
    let people = DbPerson::fetch(Some("followed=1")).await?;
    for person in people.iter() {
        let public_key: PublicKey = PublicKey::try_from_hex_string(&person.public_key)?;

        // Load which relays they use
        let person_relays =
            DbPersonRelay::fetch(Some(&format!("person='{}'", person.public_key))).await?;

        for person_relay in person_relays.iter() {
            let url: Url = Url(person_relay.relay.clone());

            let entry = hashmap.entry(url).or_default();

            entry.add_author(&public_key, None);
        }

        // If they have no relay, we will handle them next loop
    }

    // Update all the filters
    {
        for (_url, filters) in hashmap.iter_mut() {
            // Listen to these six kinds of events
            filters.add_event_kind(EventKind::Metadata);
            filters.add_event_kind(EventKind::TextNote);
            filters.add_event_kind(EventKind::RecommendRelay);
            filters.add_event_kind(EventKind::ContactList);
            filters.add_event_kind(EventKind::EventDeletion);
            filters.add_event_kind(EventKind::Reaction);

            // On startup, only pick up events in the last 12 hours
            let mut start = Unixtime::now().unwrap();
            start.0 -= 43200;

            // LETS BE NICE and not get messages from too far back
            filters.since = Some(start);

            // TODO - check the database for which events we are up to.
        }
    }

    Ok(hashmap)
}

pub async fn handle_relay(filters: Filters, url: Url) {
    // Catch errors
    if let Err(e) = handle_relay_inner(filters, url.clone()).await {
        log::error!("ERROR handling {}: {}", &url.0, e);
    }

    // Should we signal that we are exiting?
}

async fn handle_relay_inner(filters: Filters, url: Url) -> Result<(), Error> {
    log::info!("Task started to handle relay at {}", &url.0);

    log::debug!(
        "Filter for {}: {}",
        &url.0,
        serde_json::to_string(&filters)?
    );

    // Get the broadcast channel and subscribe to it
    let tx = GLOBALS.bus.clone();
    let mut rx = tx.subscribe();

    // Connect to the relay
    let (websocket_stream, _response) = tokio_tungstenite::connect_async(&url.0).await?;
    log::info!("Connected to {}", &url.0);

    let (mut write, mut read) = websocket_stream.split();

    // Subscribe to our filters
    let message = ClientMessage::Req(
        SubscriptionId("gossip-dev-testing".to_owned()),
        vec![filters.clone()],
    );
    let wire = serde_json::to_string(&message)?;
    log::debug!("About to send {}", &wire);
    write.send(WsMessage::Text(wire.clone())).await?;
    log::debug!("Sent {}", &wire);

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
                log::debug!("Handling message from {}", &url.0);
                match ws_message {
                    WsMessage::Text(t) => handle_nostr_message(tx.clone(), t, url.0.clone()).await?,
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
                if bus_message.target == url.0 {
                    log::warn!("Websocket task got message, unimpmented: {}",
                               bus_message.payload);
                } else if &*bus_message.target == "all" && &*bus_message.kind == "shutdown" {
                    log::info!("Websocket listener {} shutting down", &url.0);
                    break 'relayloop;
                }
            },
        }
    }

    Ok(())
}

async fn handle_nostr_message(
    tx: Sender<BusMessage>,
    message: String,
    urlstr: String,
) -> Result<(), Error> {
    let message: RelayMessage = serde_json::from_str(&message)?;

    match message {
        RelayMessage::Event(_subid, event) => {
            if let Err(_e) = event.verify(Some(Unixtime::now()?)) {
                log::error!("VERIFY ERROR: {:?}", serde_json::to_string(&event)?)
            } else {

                // TODO: save into the database

                if let Err(e) = tx.send(BusMessage {
                    target: "to_javascript".to_string(),
                    source: urlstr,
                    kind: "event".to_string(),
                    payload: serde_json::to_string(&event)?,
                }) {
                    log::error!("Unable to send message to javascript: {}", e);
                }
            }
        }
        RelayMessage::Notice(msg) => {
            println!("NOTICE: {}", msg);
        }
        RelayMessage::Eose(subid) => {
            println!("EOSE: {:?}", subid);
        }
        RelayMessage::Ok(id, ok, message) => {
            println!("OK: {:?} {} {}", id, ok, message);
        }
    }

    Ok(())
}
