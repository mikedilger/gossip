use super::Minion;
use crate::db::{DbEventRelay, DbRelay};
use crate::error::Error;
use crate::globals::GLOBALS;
use futures_util::sink::SinkExt;
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
                        .subscription_map
                        .get_handle_by_id(&subid.0)
                        .unwrap_or_else(|| "_".to_owned());

                    // Events that come in after EOSE on the general feed bump the last_general_eose
                    // timestamp for that relay, so we don't query before them next time we run.
                    if let Some(sub) = self.subscription_map.get_mut_by_id(&subid.0) {
                        if handle == "general_feed" && sub.eose() {
                            // set in database
                            DbRelay::update_general_eose(
                                self.dbrelay.url.clone(),
                                event.created_at.0 as u64,
                            )
                            .await?;
                            // set in globals
                            if let Some(mut dbrelay) = GLOBALS.all_relays.get_mut(&self.dbrelay.url)
                            {
                                dbrelay.last_general_eose_at = Some(event.created_at.0 as u64);
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
                }
            }
            RelayMessage::Notice(msg) => {
                tracing::warn!("{}: NOTICE: {}", &self.url, msg);
            }
            RelayMessage::Eose(subid) => {
                let handle = self
                    .subscription_map
                    .get_handle_by_id(&subid.0)
                    .unwrap_or_else(|| "_".to_owned());

                // If this is a temporary subscription, we should close it after an EOSE
                let close: bool = handle.starts_with("temp_");

                // Update the matching subscription
                match self.subscription_map.get_mut_by_id(&subid.0) {
                    Some(sub) => {
                        tracing::debug!("{}: {}: EOSE: {:?}", &self.url, handle, subid);
                        if close {
                            self.unsubscribe(&handle).await?;
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
                let url = &self.url;
                let idhex = id.as_hex_string();
                let relay_response = if !ok_message.is_empty() {
                    format!("{url}: OK={ok} id={idhex}  message=\"{ok_message}\"")
                } else {
                    format!("{url}: OK={ok} id={idhex}")
                };

                // If we are waiting for a response for this id, process
                if self.postings.contains(&id) {
                    if ok {
                        // Save seen_on data in GLOBALS.events
                        // (it was already processed by the overlord before the minion got it,
                        //  but with None for seen_on.)
                        GLOBALS.events.add_seen_on(id, &self.url);

                        // save seen_on data in database
                        let event_relay = DbEventRelay {
                            event: idhex,
                            relay: url.0.to_owned(),
                            when_seen: Unixtime::now()?.0 as u64,
                        };
                        DbEventRelay::insert(event_relay, true).await?;
                    } else {
                        // demerit the relay
                        self.bump_failure_count().await;
                    }
                    self.postings.remove(&id);
                }

                match ok {
                    true => tracing::info!("{relay_response}"),
                    false => tracing::warn!("{relay_response}"),
                }
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
                let event = GLOBALS.signer.sign_preevent(pre_event, None, None)?;
                let msg = ClientMessage::Auth(Box::new(event));
                let wire = serde_json::to_string(&msg)?;
                let ws_stream = self.stream.as_mut().unwrap();
                ws_stream.send(WsMessage::Text(wire)).await?;
                tracing::info!("Authenticated to {}", &self.url);
            }
        }

        Ok(())
    }
}
