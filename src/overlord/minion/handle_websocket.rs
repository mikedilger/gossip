use super::Minion;
use crate::globals::GLOBALS;
use crate::Error;
use futures::SinkExt;
use nostr_types::{RelayMessage, Unixtime};
use tracing::{debug, error, info, warn};
use tungstenite::protocol::Message as WsMessage;

impl Minion {
    pub(super) async fn handle_nostr_message(&mut self, ws_message: String) -> Result<(), Error> {
        // TODO: pull out the raw event without any deserialization to be sure we don't mangle
        //       it.

        let relay_message: RelayMessage = serde_json::from_str(&ws_message)?;

        let mut maxtime = Unixtime::now()?;
        maxtime.0 += 60 * 15; // 15 minutes into the future

        match relay_message {
            RelayMessage::Event(subid, event) => {
                if let Err(e) = event.verify(Some(maxtime)) {
                    error!(
                        "{}: VERIFY ERROR: {}, {}",
                        &self.url,
                        e,
                        serde_json::to_string(&event)?
                    )
                } else {
                    let handle = self
                        .subscriptions
                        .get_handle_by_id(&subid.0)
                        .unwrap_or_else(|| "_".to_owned());
                    debug!("{}: {}: NEW EVENT", &self.url, handle);

                    GLOBALS
                        .incoming_events
                        .write()
                        .await
                        .push((*event, self.url.clone()));
                }
            }
            RelayMessage::Notice(msg) => {
                info!("{}: NOTICE: {}", &self.url, msg);
            }
            RelayMessage::Eose(subid) => {
                let handle = self
                    .subscriptions
                    .get_handle_by_id(&subid.0)
                    .unwrap_or_else(|| "_".to_owned());

                let close: bool = &handle[0..5] == "event";

                // Update the matching subscription
                match self.subscriptions.get_mut_by_id(&subid.0) {
                    Some(sub) => {
                        info!("{}: {}: EOSE: {:?}", &self.url, handle, subid);
                        if close {
                            let close_message = sub.close_message();
                            let websocket_sink = self.sink.as_mut().unwrap();
                            let wire = serde_json::to_string(&close_message)?;
                            websocket_sink.send(WsMessage::Text(wire.clone())).await?;
                        } else {
                            sub.set_eose();
                        }
                    }
                    None => {
                        warn!(
                            "{}: {} EOSE for unknown subscription {:?}",
                            &self.url, handle, subid
                        );
                    }
                }
            }
            RelayMessage::Ok(id, ok, ok_message) => {
                // These don't have to be processed.
                info!(
                    "{}: OK: id={} ok={} message=\"{}\"",
                    &self.url,
                    id.as_hex_string(),
                    ok,
                    ok_message
                );
            }
        }

        Ok(())
    }
}
