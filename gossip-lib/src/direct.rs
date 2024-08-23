use crate::{Error, ErrorKind};
use base64::Engine;
use http::Uri;
use nostr_types::{ClientMessage, Event, Filter, RelayMessage, SubscriptionId};
use tungstenite::protocol::Message;

pub fn fetch(url: &str, filters: Vec<Filter>) -> Result<Vec<Event>, Error> {
    tracing::info!("Fetching from {}", url);

    let (host, uri) = url_to_host_and_uri(url)?;

    let wire = {
        let message = ClientMessage::Req(SubscriptionId("gossip_direct".to_owned()), filters);
        serde_json::to_string(&message)?
    };

    let mut events: Vec<Event> = Vec::new();

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

    let (mut websocket, _response) = tungstenite::connect(request)?;

    websocket.send(Message::Text(wire))?;

    loop {
        let message = match websocket.read() {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("Problem reading from websocket: {}", e);
                return Ok(events);
            }
        };

        match message {
            Message::Text(s) => {
                tracing::debug!("RAW MESSAGE: {}", s);
                let relay_message: RelayMessage = serde_json::from_str(&s)?;
                match relay_message {
                    RelayMessage::Event(_, e) => events.push(*e),
                    RelayMessage::Notice(s) => tracing::info!("NOTICE: {}", s),
                    RelayMessage::Notify(s) => tracing::info!("NOTIFY: {}", s),
                    RelayMessage::Eose(_) => {
                        let message = ClientMessage::Close(SubscriptionId("111".to_owned()));
                        let wire = match serde_json::to_string(&message) {
                            Ok(w) => w,
                            Err(e) => {
                                tracing::error!("Could not serialize message: {}", e);
                                return Ok(events);
                            }
                        };
                        if let Err(e) = websocket.send(Message::Text(wire)) {
                            tracing::error!("Could not write close subscription message: {}", e);
                            return Ok(events);
                        }
                        if let Err(e) = websocket.send(Message::Close(None)) {
                            tracing::error!("Could not write websocket close message: {}", e);
                            return Ok(events);
                        }
                    }
                    RelayMessage::Ok(_id, ok, reason) => {
                        tracing::info!("OK: ok={} reason={}", ok, reason)
                    }
                    RelayMessage::Auth(challenge) => {
                        // FIXME
                        tracing::info!("AUTH: {}", challenge)
                    }
                    RelayMessage::Closed(_, reason) => {
                        tracing::info!("CLOSED: {}", reason)
                    }
                }
            }
            Message::Binary(_) => tracing::debug!("IGNORING BINARY MESSAGE"),
            Message::Ping(vec) => {
                if let Err(e) = websocket.send(Message::Pong(vec)) {
                    tracing::warn!("Unable to pong: {}", e);
                }
            }
            Message::Pong(_) => tracing::debug!("IGNORING PONG"),
            Message::Close(_) => {
                tracing::debug!("Closing");
                break;
            }
            Message::Frame(_) => tracing::debug!("UNEXPECTED RAW WEBSOCKET FRAME"),
        }
    }

    Ok(events)
}

pub fn post(url: &str, event: Event) -> Result<(), Error> {
    tracing::info!("Posting to {}", url);

    let (host, uri) = url_to_host_and_uri(url)?;

    let wire = {
        let message = ClientMessage::Event(Box::new(event));
        serde_json::to_string(&message)?
    };

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

    let (mut websocket, _response) = tungstenite::connect(request)?;

    websocket.send(Message::Text(wire))?;

    // Get and print one response message

    let message = match websocket.read() {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("Problem reading from websocket: {}", e);
            return Ok(());
        }
    };

    match message {
        Message::Text(s) => {
            let relay_message: RelayMessage = serde_json::from_str(&s)?;
            match relay_message {
                RelayMessage::Event(_, e) => {
                    let event = serde_json::to_string(&e)?;
                    tracing::info!("EVENT: {}", event);
                }
                RelayMessage::Notice(s) => tracing::info!("NOTICE: {}", s),
                RelayMessage::Notify(s) => tracing::info!("NOTIFY: {}", s),
                RelayMessage::Eose(_) => tracing::info!("EOSE"),
                RelayMessage::Ok(_id, ok, reason) => {
                    tracing::info!("OK: ok={} reason={}", ok, reason)
                }
                RelayMessage::Auth(challenge) => tracing::info!("AUTH: {}", challenge),
                RelayMessage::Closed(_, message) => tracing::info!("CLOSED: {}", message),
            }
        }
        Message::Binary(_) => tracing::debug!("IGNORING BINARY MESSAGE"),
        Message::Ping(vec) => {
            if let Err(e) = websocket.send(Message::Pong(vec)) {
                tracing::warn!("Unable to pong: {}", e);
            }
        }
        Message::Pong(_) => tracing::debug!("IGNORING PONG"),
        Message::Close(_) => {
            tracing::info!("Closing");
            return Ok(());
        }
        Message::Frame(_) => tracing::debug!("UNEXPECTED RAW WEBSOCKET FRAME"),
    }

    Ok(())
}

fn url_to_host_and_uri(url: &str) -> Result<(String, Uri), Error> {
    let uri: http::Uri = url.parse::<http::Uri>()?;
    let authority = match uri.authority() {
        Some(auth) => auth.as_str(),
        None => return Err(ErrorKind::UrlHasNoHostname.into()),
    };
    let host = authority
        .find('@')
        .map(|idx| authority.split_at(idx + 1).1)
        .unwrap_or_else(|| authority);
    if host.is_empty() {
        Err(ErrorKind::UrlHasEmptyHostname.into())
    } else {
        Ok((host.to_owned(), uri))
    }
}
