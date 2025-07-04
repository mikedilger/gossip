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
                    //
                    // exclude temp_ feeds that get rewritten as we are likely to get data
                    // from the previous version of said filter.
                    if !handle.starts_with("temp_") {
                        let mut it_matches = false;
                        if sub.get_filter().event_matches(&event) {
                            it_matches = true;
                        }
                        if !it_matches {
                            tracing::debug!(
                                "{} sent event that does not match filters on subscription {}: {}",
                                self.url,
                                handle,
                                event.id.as_hex_string()
                            );

                            // ignore it
                            return Ok(());
                        }
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
                        GLOBALS.db().modify_relay(
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
            }
            RelayMessage::Notify(msg) => {
                let message = format!("{}: NOTIFY: {}", &self.url, msg);
                tracing::warn!(message);

                GLOBALS
                    .pending
                    .insert(crate::pending::PendingItem::NotifyMessage(message));
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
                            let now = Unixtime::now().0 as u64;
                            self.dbrelay.last_general_eose_at =
                                Some(match self.dbrelay.last_general_eose_at {
                                    Some(old) => old.max(now),
                                    None => now,
                                });
                            GLOBALS.db().modify_relay(
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

                match ok {
                    true => tracing::debug!("{relay_response}"),
                    false => tracing::info!("{relay_response}"),
                }

                // If this is in response to our AUTH event...
                if let AuthState::Waiting(waiting_id) = self.auth_state {
                    if waiting_id == id {
                        if !ok {
                            self.auth_state = AuthState::Failed;
                            // Auth failed.
                            tracing::warn!("AUTH failed to {}: {}", &self.url, ok_message);
                        } else {
                            tracing::info!("Authenticated to {}", &self.url);
                            self.auth_state = AuthState::Authenticated;
                            self.try_subscribe_waiting().await?;
                            self.resend_post_auth().await?;
                        }
                        return Ok(());
                    }
                }
                // If this is in response to our fake AUTH event...
                else if let AuthState::FakeWaiting(waiting_id) = self.auth_state {
                    if waiting_id == id {
                        if !ok {
                            self.auth_state = AuthState::Failed;
                            // Auth failed.
                            tracing::warn!("fake-AUTH failed to {}: {}", &self.url, ok_message);
                        } else {
                            tracing::info!("Fake-authenticated to {}", &self.url);
                            self.auth_state = AuthState::FakeAuthenticated;
                            self.try_subscribe_waiting().await?;
                            self.resend_post_auth().await?;
                        }
                        return Ok(());
                    }
                }
                // Else if this is in response to an EVENT we did earlier
                else if let Some(job_id) = self.posting_ids.get(&id).copied() {
                    if ok {
                        // Save seen_on data
                        // (it was already processed by the overlord before the minion got it,
                        //  but with None for seen_on.)
                        GLOBALS.db().add_event_seen_on_relay(
                            id,
                            &self.url,
                            Unixtime::now(),
                            None,
                        )?;
                    } else {
                        // If 'auth-required', then trigger auth and remember to repost this event
                        // afterwards:
                        if ok_message.starts_with("auth-required:") {
                            self.repost_ids_after_auth.insert(id); // retry EVENT after auth
                            self.maybe_authenticate().await?;
                            return Ok(());
                        } else {
                            // else demerit the relay
                            self.bump_failure_count().await;
                        }
                    }

                    let mut job_is_done: bool = false;
                    {
                        // Take it out of the posting_jobs
                        if let Some(job_ids) = self.posting_jobs.get_mut(&job_id) {
                            job_ids.retain(|id_in_vec| *id_in_vec != id);
                            job_is_done = job_ids.is_empty();
                        } // else is not expected, but if it happens we ignore it.
                    }

                    if job_is_done {
                        self.posting_jobs.remove(&job_id);

                        // Tell overlord
                        self.to_overlord.send(ToOverlordMessage::MinionJobComplete(
                            self.url.clone(),
                            job_id,
                        ))?;
                    }

                    // Take it out of the posting_ids whether or not job is done
                    self.posting_ids.remove(&id);
                }
            }
            RelayMessage::Auth(challenge) => {
                if self.auth_state.is_authenticated() || self.auth_state.failed() {
                    // Ignore the AUTH. We already did.
                    return Ok(());
                }

                self.auth_challenge = challenge.to_owned();
                self.maybe_authenticate().await?;
            }
            RelayMessage::Closed(subid, message) => {
                let handle = self
                    .subscription_map
                    .get_handle_by_id(&subid.0)
                    .unwrap_or_else(|| "_".to_owned());

                if handle != "_" {
                    tracing::info!("{}: Closed: {}: {}", &self.url, handle, message);
                }

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
                                        self.maybe_authenticate().await?;

                                        // cork and retry once auth completes
                                        self.subscriptions_waiting_for_auth
                                            .insert(handle, Unixtime::now());

                                        // return now, don't remove sub from map
                                        return Ok(());
                                    }
                                    AuthState::Waiting(_) | AuthState::FakeWaiting(_) => {
                                        // cork and retry once auth completes
                                        self.subscriptions_waiting_for_auth
                                            .insert(handle, Unixtime::now());

                                        // return now, don't remove sub from map
                                        return Ok(());
                                    }
                                    AuthState::Authenticated | AuthState::FakeAuthenticated => {
                                        // We are authenticated, but it doesn't think so.
                                        // The relay is broken. Fail this sub.
                                        self.failed_subs.insert(handle.clone());
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
                            tracing::debug!("{} closed with unknown prefix {}", &self.url, prefix);
                            // Presume any other kind of Closed is an failed subscription
                            self.failed_subs.insert(handle.clone());
                        }
                    }
                }

                // Remove the subscription
                tracing::debug!("{}: removed subscription {}", &self.url, handle);
                self.subscription_map.remove(&handle);
            }
            RelayMessage::Count(_subid, _count_result) => {
                tracing::info!(
                    "{} Relay returned count results that we do not yet support",
                    &self.url
                );
            }
        }

        Ok(())
    }
}
