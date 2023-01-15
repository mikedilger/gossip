use super::Minion;
use crate::db::DbRelay;
use crate::error::Error;
use futures::SinkExt;
use nostr_types::{RelayMessage, Unixtime};
use tungstenite::protocol::Message as WsMessage;

impl Minion {
    pub(super) async fn handle_nostr_message(&mut self, ws_message: String) -> Result<(), Error> {
        // TODO: pull out the raw event without any deserialization to be sure we don't mangle
        //       it.

        let relay_message: RelayMessage = match serde_json::from_str(&ws_message) {
            Ok(rm) => rm,
            Err(e) => {
                tracing::error!(
                    "RELAY MESSAGE NOT DESERIALIZING: starts with \"{}\"",
                    &ws_message.chars().take(300).collect::<String>()
                );
                return Err(e.into());
            }
        };

        let mut maxtime = Unixtime::now()?;
        maxtime.0 += 60 * 15; // 15 minutes into the future

        match relay_message {
            RelayMessage::Event(subid, event) => {
                if let Err(e) = event.verify(Some(maxtime)) {
                    tracing::error!(
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

                    tracing::debug!("{}: {}: New Event: {:?}", &self.url, handle, event.kind);

                    // Events that come in after EOSE on the general feed bump the last_general_eose
                    // timestamp for that relay, so we don't query before them next time we run.
                    if let Some(sub) = self.subscriptions.get_mut_by_id(&subid.0) {
                        if handle == "general_feed" && sub.eose() {
                            DbRelay::update_general_eose(
                                self.dbrelay.url.clone(),
                                event.created_at.0 as u64,
                            )
                            .await?;
                        }
                    }

                    // Try processing everything immediately
                    crate::process::process_new_event(
                        &event,
                        true,
                        Some(self.url.clone()),
                        Some(handle),
                    )
                    .await?;

                    /*
                    if event.kind == EventKind::TextNote {
                        // Just store text notes in incoming
                        GLOBALS
                            .incoming_events
                            .write()
                            .await
                            .push((*event, self.url.clone(), handle));
                    } else {
                        // Process everything else immediately
                        crate::process::process_new_event(&event, true, Some(self.url.clone()))
                            .await?;
                    }
                     */
                }
            }
            RelayMessage::Notice(msg) => {
                tracing::info!("{}: NOTICE: {}", &self.url, msg);
            }
            RelayMessage::Eose(subid) => {
                let handle = self
                    .subscriptions
                    .get_handle_by_id(&subid.0)
                    .unwrap_or_else(|| "_".to_owned());

                let close: bool = handle.starts_with("temp_");

                // Update the matching subscription
                match self.subscriptions.get_mut_by_id(&subid.0) {
                    Some(sub) => {
                        tracing::debug!("{}: {}: EOSE: {:?}", &self.url, handle, subid);
                        if close {
                            let close_message = sub.close_message();
                            let websocket_sink = self.sink.as_mut().unwrap();
                            let wire = serde_json::to_string(&close_message)?;
                            websocket_sink.send(WsMessage::Text(wire.clone())).await?;
                        } else {
                            sub.set_eose();
                        }
                        if handle == "general_feed" {
                            let now = Unixtime::now().unwrap().0 as u64;
                            DbRelay::update_general_eose(self.dbrelay.url.clone(), now).await?;
                        }
                    }
                    None => {
                        tracing::debug!(
                            "{}: {} EOSE for unknown subscription {:?}",
                            &self.url,
                            handle,
                            subid
                        );
                    }
                }
            }
            RelayMessage::Ok(id, ok, ok_message) => {
                // These don't have to be processed.
                tracing::info!(
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
