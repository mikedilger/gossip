use super::{AuthState, Minion};
use crate::comms::ToOverlordMessage;
use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{RelayMessage, Unixtime};

impl Minion {
    pub(super) async fn handle_nostr_message(&mut self, ws_message: String) -> Result<(), Error> {
        // TODO: pull out the raw event without any deserialization to be sure we don't mangle
        //       it.

        let relay_message: RelayMessage = match serde_json::from_str(&ws_message) {
            Ok(rm) => rm,
            Err(e) => {
                tracing::warn!(
                    "RELAY MESSAGE NOT DESERIALIZING ({}) ({}): starts with \"{}\"",
                    self.url,
                    e,
                    &ws_message.chars().take(300).collect::<String>()
                );
                return Err(e.into());
            }
        };

        match relay_message {
            RelayMessage::Event(subid, event) => {
                let handle = self
                    .subscription_map
                    .get_handle_by_id(&subid.0)
                    .unwrap_or_else(|| "_".to_owned());

                if let Some(sub) = self.subscription_map.get_mut_by_id(&subid.0) {
                    // Check if the event matches one of our filters
                    // and ignore it if it doesn't
                    let mut it_matches = false;
                    for filter in sub.get_filters().iter() {
                        if filter.event_matches_incomplete(&event) {
                            it_matches = true;
                            break;
                        }
                    }
                    if !it_matches {
                        tracing::info!(
                            "{} sent event that does not match filters on subscription {}: {}",
                            self.url,
                            handle,
                            event.id.as_hex_string()
                        );
                        return Ok(());
                    }

                    // Events that come in after EOSE on the general feed bump the last_general_eose
                    // timestamp for that relay, so we don't query before them next time we run.
                    if handle == "general_feed" && sub.eose() {
                        // Update last general EOSE
                        self.dbrelay.last_general_eose_at =
                            Some(match self.dbrelay.last_general_eose_at {
                                Some(old) => old.max(event.created_at.0 as u64),
                                None => event.created_at.0 as u64,
                            });
                        GLOBALS.storage.modify_relay(
                            &self.dbrelay.url,
                            |relay| {
                                relay.last_general_eose_at = self.dbrelay.last_general_eose_at;
                            },
                            None,
                        )?;
                    }
                }

                // Remove from sought set
                if let Some(ess) = self.sought_events.remove(&event.id) {
                    // and notify the overlord of the completed job
                    for job_id in ess.job_ids.iter() {
                        self.to_overlord.send(ToOverlordMessage::MinionJobComplete(
                            self.url.clone(),
                            *job_id,
                        ))?;
                    }
                }

                // Process the event
                crate::process::process_new_event(
                    &event,
                    Some(self.url.clone()),
                    Some(handle),
                    true,
                    false,
                )
                .await?;
            }
            RelayMessage::Notice(msg) => {
                tracing::warn!("{}: NOTICE: {}", &self.url, msg);
                tracing::warn!(
                    "{}: last message sent was: {}",
                    &self.url,
                    &self.last_message_sent
                );
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
                            // Update last general EOSE
                            let now = Unixtime::now().unwrap().0 as u64;
                            self.dbrelay.last_general_eose_at =
                                Some(match self.dbrelay.last_general_eose_at {
                                    Some(old) => old.max(now),
                                    None => now,
                                });
                            GLOBALS.storage.modify_relay(
                                &self.dbrelay.url,
                                |relay| {
                                    relay.last_general_eose_at = self.dbrelay.last_general_eose_at;
                                },
                                None,
                            )?;
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
                if let AuthState::Waiting(waiting_id) = self.auth_state {
                    if waiting_id == id {
                        if !ok {
                            self.auth_state = AuthState::Failed;
                            // Auth failed.
                            tracing::warn!("AUTH failed to {}: {}", &self.url, ok_message);
                        } else {
                            self.auth_state = AuthState::Authenticated;
                            self.try_subscribe_waiting().await?;
                        }
                    }
                } else if self.postings.contains(&id) {
                    if ok {
                        // Save seen_on data
                        // (it was already processed by the overlord before the minion got it,
                        //  but with None for seen_on.)
                        GLOBALS.storage.add_event_seen_on_relay(
                            id,
                            &self.url,
                            Unixtime::now().unwrap(),
                            None,
                        )?;
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
                self.auth_challenge = challenge.to_owned();
                if GLOBALS.storage.read_setting_relay_auth_requires_approval() {
                    match self.dbrelay.allow_auth {
                        Some(true) => self.authenticate().await?,
                        Some(false) => (),
                        None => GLOBALS.auth_requests.write().push(self.url.clone()),
                    }
                } else {
                    self.authenticate().await?
                }
            }
            RelayMessage::Closed(subid, message) => {
                let handle = self
                    .subscription_map
                    .get_handle_by_id(&subid.0)
                    .unwrap_or_else(|| "_".to_owned());

                tracing::info!("{}: Closed: {}: {}", &self.url, handle, message);

                // Check the machine-readable prefix
                if let Some(prefix) = message.split(':').next() {
                    match prefix {
                        "duplicate" => {
                            // not much we can do; it SHOULD replace dup REQ subs, not complain.
                            tracing::warn!(
                                "{} not accepting {} due to duplicate is strange.",
                                &self.url,
                                handle
                            );
                        }
                        "pow" => {
                            tracing::warn!(
                                "{} wants POW for {} but we do not do POW on demand.",
                                &self.url,
                                handle
                            );
                        }
                        "rate-limited" => {
                            // Wait to retry later
                            self.subscriptions_rate_limited.push(handle);

                            // return now, don't remove sub from map
                            return Ok(());
                        }
                        "invalid" => {
                            tracing::warn!(
                                "{} won't serve our {} sub (says invalid)",
                                &self.url,
                                &handle
                            );
                            self.failed_subs.insert(handle.clone());
                        }
                        "error" => {
                            tracing::warn!(
                                "{} won't serve our {} sub (says error)",
                                &self.url,
                                &handle
                            );
                            self.failed_subs.insert(handle.clone());
                        }
                        "auth-required" => {
                            if self.dbrelay.allow_auth == Some(false) {
                                // we don't allow auth to this relay.
                                // fail this subscription handle
                                self.failed_subs.insert(handle.clone());
                            } else {
                                match self.auth_state {
                                    AuthState::None => {
                                        // authenticate
                                        self.authenticate().await?;

                                        // cork and retry once auth completes
                                        self.subscriptions_waiting_for_auth
                                            .push((handle, Unixtime::now().unwrap()));

                                        // return now, don't remove sub from map
                                        return Ok(());
                                    }
                                    AuthState::Waiting(_) => {
                                        // cork and retry once auth completes
                                        self.subscriptions_waiting_for_auth
                                            .push((handle, Unixtime::now().unwrap()));

                                        // return now, don't remove sub from map
                                        return Ok(());
                                    }
                                    AuthState::Authenticated => {
                                        // We are authenticated, but it doesn't think so.
                                        // Presume it is a race condition and ignore it.
                                        // (fall through, it will be removed, but subsequent
                                        //  similar subs will not be listed in failed_subs)
                                    }
                                    AuthState::Failed => {
                                        // fail this subscription handle
                                        self.failed_subs.insert(handle.clone());
                                    }
                                }
                            }
                        }
                        "restricted" => {
                            tracing::warn!(
                                "{} won't serve our {} sub (says restricted)",
                                &self.url,
                                &handle
                            );
                            self.failed_subs.insert(handle.clone());
                        }
                        _ => {
                            tracing::warn!("{} closed with unknown prefix {}", &self.url, prefix);
                        }
                    }
                }

                // Remove the subscription
                tracing::info!("{}: removed subscription {}", &self.url, handle);
                let _ = self.subscription_map.remove(&handle);
            }
        }

        Ok(())
    }
}
