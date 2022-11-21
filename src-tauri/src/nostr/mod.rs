use crate::db::{DbEvent, DbEventSeen, DbEventTag, DbPerson, DbPersonRelay, DbRelay};
use crate::{BusMessage, Error, GLOBALS};
use futures::{SinkExt, StreamExt};
use nostr_proto::{
    ClientMessage, Event, EventKind, Filters, Metadata, PublicKey, RelayMessage,
    SubscriptionId, Unixtime, Url,
};
use serde::Serialize;
use std::collections::HashMap;
use tokio::select;
use tokio::sync::broadcast::Sender;
use tungstenite::protocol::Message as WsMessage;

/// This function computes which relays we need to follow and what filters
/// they should have, only for startup, based on what is in the database.
pub async fn load_initial_relay_filters() -> Result<HashMap<Url, Filters>, Error> {

    // Start collecting filters per-relay
    let mut per_relay_filters: HashMap<Url, Filters> = HashMap::new();

    // Build a hashmap of relays that we know
    let mut relays = DbRelay::fetch(None).await?;
    let mut relaymap: HashMap<String, DbRelay> = HashMap::new();
    for relay in relays.drain(..) {
        relaymap.insert(relay.url.clone(), relay);
    }

    // Load all the people we are following
    let people = DbPerson::fetch(Some("followed=1")).await?;

    // Remember people for which we have no relay information
    let mut orphan_pubkeys: Vec<String> = Vec::new();

    for person in people.iter() {

        let public_key: PublicKey = PublicKey::try_from_hex_string(&person.public_key)?;

        // Load which relays they use
        let person_relays =
            DbPersonRelay::fetch(Some(&format!("person='{}'", person.public_key))).await?;

        // Get the highest ranked relay that they use
        let best_relay: Option<DbRelay> = person_relays.iter()
            .map_while(|pr| relaymap.get(&pr.relay))
            .fold(None, |current, candidate| {
                if let Some(cur) = current {
                    if cur.rank >= candidate.rank { Some(cur) }
                    else { Some(candidate.clone()) }
                } else {
                    Some(candidate.clone())
                }
            });

        if let Some(relay) = best_relay {
            let url: Url = Url(relay.url.clone());
            let entry = per_relay_filters.entry(url).or_default();
            entry.add_author(&public_key, None);
        } else {
            // if they have no relay, mark them as an orphan
            orphan_pubkeys.push(person.public_key.clone())
        }
    }

    // Listen to orphans on all relays we are already listening on
    for orphan in orphan_pubkeys.iter() {
        for (_url, filters) in per_relay_filters.iter_mut() {
            let pubkey = PublicKey::try_from_hex_string(orphan)?;
            filters.add_author(&pubkey, None);
        }
    }

    // Update all the filters
    {
        for (url, filters) in per_relay_filters.iter_mut() {

            log::debug!("We will listen to {}, {:?}", &url.0, filters.authors);

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

    for (url, filters) in per_relay_filters.iter() {
        log::info!("WILL WATCH {} WITH {:?}", &url.0, filters);
    }

    Ok(per_relay_filters)
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
        SubscriptionId(format!("gossip_{}", textnonce::TextNonce::new())),
        vec![filters.clone()],
    );
    let wire = serde_json::to_string(&message)?;
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
                    WsMessage::Text(t) => {
                        if let Err(e) = handle_nostr_message(tx.clone(), t, url.0.clone()).await {
                            log::error!("Error on {}: {}", &url.0, e);
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
    ws_message: String,
    urlstr: String,
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
                save_event_in_database(&event, urlstr.clone()).await?;
                process_event(&tx, *event, urlstr.clone()).await?;
            }
        }
        RelayMessage::Notice(msg) => {
            log::info!("NOTICE: {} {}", &urlstr, msg);
        }
        RelayMessage::Eose(subid) => {
            // These don't have to be processed.
            log::info!("EOSE: {} {:?}", &urlstr, subid);
        }
        RelayMessage::Ok(id, ok, ok_message) => {
            // These don't have to be processed.
            log::info!("OK: {} {:?} {} {}", &urlstr, id, ok, ok_message);
        }
    }

    Ok(())
}

#[derive(Serialize)]
struct Jsevent {
    id: String,
    pubkey: String,
    created_at: i64,
    kind: u64,
    content: String,
    name: String,
    avatar_url: String,
}

async fn save_event_in_database(
    event: &Event,
    urlstr: String
) -> Result<(), Error> {
    let db_event = DbEvent {
        id: event.id.as_hex_string(),
        raw: serde_json::to_string(&event)?, // TODO: this is reserialized.
        public_key: event.pubkey.as_hex_string(),
        created_at: event.created_at.0,
        kind: From::from(event.kind),
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
        relay: urlstr.clone(),
        when_seen: Unixtime::now()?.0 as u64
    };
    DbEventSeen::replace(db_event_seen).await?;

    Ok(())
}

async fn send_event_to_javascript(
    tx: &Sender<BusMessage>,
    event: Event,
    urlstr: String
) -> Result<(), Error> {

    // Event doesn't include petname
    // Look up their petname
    let maybe_db_person = DbPerson::fetch_one(event.pubkey.clone()).await?;

    let (name, avatar_url) = match maybe_db_person {
        None => ("".to_owned(), "".to_owned()),
        Some(person) => ( person.name.unwrap_or("".to_owned()),
                          person.picture.unwrap_or("".to_owned()) )
    };

    let jsevent = Jsevent { // see below for type
        id: event.id.as_hex_string(),
        pubkey: event.pubkey.as_hex_string(),
        created_at: event.created_at.0,
        kind: From::from(event.kind),
        content: event.content.clone(),
        name: name,
        avatar_url: avatar_url,
    };

    if let Err(e) = tx.send(BusMessage {
        target: "to_javascript".to_string(),
        source: urlstr.clone(),
        kind: "event".to_string(),
        payload: serde_json::to_string(&jsevent)?,
    }) {
        log::error!("Unable to send message to javascript: {}", e);
    }

    Ok(())
}

async fn process_event(
    tx: &Sender<BusMessage>,
    event: Event,
    urlstr: String
) -> Result<(), Error> {

    match event.kind {
        EventKind::Metadata => {
            let metadata: Metadata = serde_json::from_str(&event.content)?;
            if let Some(mut person) = DbPerson::fetch_one(event.pubkey.clone()).await? {
                person.name = Some(metadata.name);
                person.about = metadata.about;
                person.picture = metadata.picture;
                person.nip05 = metadata.nip05;
                DbPerson::update(person).await?;
            } else {
                let person = DbPerson {
                    public_key: event.pubkey.as_hex_string(),
                    name: Some(metadata.name),
                    about: metadata.about,
                    picture: metadata.picture,
                    nip05: metadata.nip05,
                    followed: 0
                };
                DbPerson::insert(person).await?;
            }
            // Javascript needs to update metadata on its list of events:
            send_event_to_javascript(&tx, event, urlstr.clone()).await?;
        },
        EventKind::TextNote => {
            // Javascript needs to render this event on the feed:
            send_event_to_javascript(&tx, event, urlstr.clone()).await?;
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
