use super::Minion;
use crate::db::DbRelay;
use crate::error::Error;
use crate::globals::GLOBALS;
use futures::SinkExt;
use nostr_types::{ClientMessage, EventKind, PreEvent, RelayMessage, Tag, Unixtime};
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
                            // set in database
                            DbRelay::update_general_eose(
                                self.dbrelay.url.clone(),
                                event.created_at.0 as u64,
                            )
                            .await?;
                            // set in globals
                            if let Some(mut relayinfo) = GLOBALS.relays.get_mut(&self.dbrelay.url) {
                                relayinfo.dbrelay.last_general_eose_at =
                                    Some(event.created_at.0 as u64);
                            }
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

                // If this is a temporary subscription, we should close it after an EOSE
                let close: bool = handle.starts_with("temp_");

                // Update the matching subscription
                match self.subscriptions.get_mut_by_id(&subid.0) {
                    Some(sub) => {
                        tracing::debug!("{}: {}: EOSE: {:?}", &self.url, handle, subid);
                        if close {
                            self.unsubscribe(&handle).await?;
                            // If that was the last (temp_) subscription, set minion to exit
                            if self.subscriptions.is_empty() {
                                self.keepgoing = false;
                            }
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
            RelayMessage::Auth(challenge) => {
                if !GLOBALS.signer.is_ready() {
                    tracing::warn!("AUTH required on {}, but we have no key", &self.url);
                    return Ok(());
                }
                let pubkey = match GLOBALS.signer.public_key() {
                    Some(pk) => pk,
                    None => return Ok(()),
                };
                let pre_event = PreEvent {
                    pubkey,
                    created_at: Unixtime::now().unwrap(),
                    kind: EventKind::Auth,
                    tags: vec![
                        Tag::Other {
                            tag: "relay".to_string(),
                            data: vec![self.url.0.to_owned()],
                        },
                        Tag::Other {
                            tag: "challenge".to_string(),
                            data: vec![challenge],
                        },
                    ],
                    content: "".to_string(),
                    ots: None,
                };
                let event = GLOBALS.signer.sign_preevent(pre_event, None)?;
                let msg = ClientMessage::Auth(Box::new(event));
                let wire = serde_json::to_string(&msg)?;
                let ws_sink = self.sink.as_mut().unwrap();
                ws_sink.send(WsMessage::Text(wire)).await?;
                tracing::info!("Authenticated to {}", &self.url);
            }
        }

        Ok(())
    }
}
