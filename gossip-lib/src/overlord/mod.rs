mod minion;

use crate::comms::{
    RelayConnectionReason, RelayJob, ToMinionMessage, ToMinionPayload, ToMinionPayloadDetail,
    ToOverlordMessage,
};
use crate::dm_channel::DmChannel;
use crate::error::{Error, ErrorKind};
use crate::feed::FeedKind;
use crate::globals::GLOBALS;
use crate::misc::{Private, ZapState};
use crate::nip46::{Approval, ParsedCommand};
use crate::pending::PendingItem;
use crate::people::{Person, PersonList};
use crate::relay;
use crate::relay::Relay;
use crate::storage::{PersonTable, Table};
use crate::RunState;
use dashmap::mapref::entry::Entry;
use gossip_relay_picker::RelayAssignment;
use heed::RwTxn;
use http::StatusCode;
use minion::{Minion, MinionExitReason};
use nostr_types::{
    EncryptedPrivateKey, Event, EventAddr, EventKind, EventReference, Filter, Id, IdHex, Metadata,
    MilliSatoshi, NostrBech32, PayRequestData, PreEvent, PrivateKey, Profile, PublicKey, RelayUrl,
    RelayUsage, Tag, UncheckedUrl, Unixtime,
};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;
#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};
#[cfg(windows)]
use tokio::signal::windows::{ctrl_break, ctrl_c, ctrl_close};
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::watch::Receiver as WatchReceiver;
use tokio::task;
use zeroize::Zeroize;

type MinionResult = Result<MinionExitReason, Error>;

/// The overlord handles any operation that involves talking to relays, and a few more.
///
/// There are two ways to engage the Overlord to do something:
///
/// 1. Call a function on it. This works from an async context.
/// 2. Send it a message using `GLOBALS.to_overlord`. This works from a synchronous
///    context, but does not wait for or deliver a result. This is how the canonical
///    immediate-mode renderer (egui) engages the Overlord.
pub struct Overlord {
    to_minions: Sender<ToMinionMessage>,
    inbox: UnboundedReceiver<ToOverlordMessage>,

    read_runstate: WatchReceiver<RunState>,

    // All the minion tasks running.
    minions: task::JoinSet<Result<MinionExitReason, Error>>,

    // Map from minion task::Id to Url
    minions_task_url: HashMap<task::Id, RelayUrl>,
}

impl Overlord {
    /// To create an Overlord (and you should really only create one, even though we have
    /// not forced this to be a singleton), you'll want to call this `new` function and
    /// pass one half of the unbounded_channel to the overlord. You will have to steal this
    /// from GLOBALS as follows:
    ///
    /// ```no_run
    /// # use std::ops::DerefMut;
    /// # #[tokio::main]
    /// # async fn main() {
    /// #   use gossip_lib::GLOBALS;
    /// let overlord_receiver = {
    ///   let mut mutex_option = GLOBALS.tmp_overlord_receiver.lock().await;
    ///   mutex_option.deref_mut().take()
    /// }.unwrap();
    ///
    /// let mut overlord = gossip_lib::Overlord::new(overlord_receiver);
    /// # }
    /// ```
    ///
    /// Once you have created an overlord, run it and await on it. This will block your thread.
    /// You may use other `tokio` or `futures` combinators, or spawn it on it's own thread
    /// if you wish.
    ///
    /// ```no_run
    /// # use std::ops::DerefMut;
    /// # #[tokio::main]
    /// # async fn main() {
    /// #   use gossip_lib::GLOBALS;
    /// #   let overlord_receiver = {
    /// #     let mut mutex_option = GLOBALS.tmp_overlord_receiver.lock().await;
    /// #     mutex_option.deref_mut().take()
    /// #   }.unwrap();
    /// #
    /// #   let mut overlord = gossip_lib::Overlord::new(overlord_receiver);
    /// overlord.run().await;
    /// # }
    /// ```
    pub fn new(inbox: UnboundedReceiver<ToOverlordMessage>) -> Overlord {
        let to_minions = GLOBALS.to_minions.clone();
        Overlord {
            to_minions,
            inbox,
            read_runstate: GLOBALS.read_runstate.clone(),
            minions: task::JoinSet::new(),
            minions_task_url: HashMap::new(),
        }
    }

    /// This runs the overlord. This blocks for the entire duration and only exits
    /// when the overlord receives a signal to shutdown.
    pub async fn run(&mut self) {
        if let Err(e) = self.run_inner().await {
            tracing::error!("{}", e);
        }

        if let Err(e) = GLOBALS.storage.sync() {
            tracing::error!("{}", e);
        } else {
            tracing::info!("LMDB synced.");
        }

        let _ = GLOBALS.write_runstate.send(RunState::ShuttingDown);

        tracing::info!("Overlord waiting for minions to all shutdown");

        // Listen on self.minions until it is empty
        while !self.minions.is_empty() {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(10)) => {
                    tracing::info!("Minions are stuck. Shutting down anyways.");
                    break;
                },
                task_nextjoined = self.minions.join_next_with_id() => {
                    self.handle_task_nextjoined(task_nextjoined).await;
                }
            }
        }

        tracing::info!("Overlord confirms all minions have shutdown");
    }

    async fn run_inner(&mut self) -> Result<(), Error> {
        // Maybe wait for UI login
        if GLOBALS.wait_for_login.load(Ordering::Relaxed) {
            GLOBALS.wait_for_login_notify.notified().await;
        }

        // Check for shutdown (we might not have gotten a login)
        if *self.read_runstate.borrow() == RunState::ShuttingDown {
            return Ok(());
        }

        {
            // If we need to rebuild relationships, do so now
            if GLOBALS.storage.get_flag_rebuild_relationships_needed() {
                tracing::info!("Rebuilding relationships...");
                GLOBALS.storage.rebuild_relationships(None)?;
            }

            // If we need to rebuild indexes, do so now
            if GLOBALS.storage.get_flag_rebuild_indexes_needed() {
                tracing::info!("Rebuilding event indices...");
                GLOBALS.storage.rebuild_event_indices(None)?;
            }

            // If we need to reapply relay lists, do so now
            if GLOBALS.storage.get_flag_reprocess_relay_lists_needed() {
                tracing::info!("Reprocessing relay lists...");
                crate::process::reprocess_relay_lists()?;
            }

            // Data migrations complete
            GLOBALS
                .wait_for_data_migration
                .store(false, Ordering::Relaxed);
        }

        // Switch out of initializing RunState
        if GLOBALS.storage.read_setting_offline() {
            let _ = GLOBALS.write_runstate.send(RunState::Offline);
        } else {
            if *GLOBALS.read_runstate.borrow() != RunState::ShuttingDown {
                let _ = GLOBALS.write_runstate.send(RunState::Online);
            }
        }

        #[cfg(unix)]
        let mut interrupt_signal = signal(SignalKind::interrupt())?;
        #[cfg(unix)]
        let mut quit_signal = signal(SignalKind::quit())?;
        #[cfg(unix)]
        let mut terminate_signal = signal(SignalKind::terminate())?;

        #[cfg(windows)]
        let mut interrupt_signal = ctrl_c()?;
        #[cfg(windows)]
        let mut quit_signal = ctrl_break()?;
        #[cfg(windows)]
        let mut terminate_signal = ctrl_close()?;

        // Start background tasks
        crate::tasks::start_background_tasks();

        'mainloop: loop {
            tracing::debug!("overlord looping");

            // Listen on inbox, runstate, and exiting minions
            tokio::select! {
                message = self.inbox.recv() => {
                    let message = match message {
                        Some(bm) => bm,
                        None => {
                            // All senders dropped, or one of them closed.
                            let _ = GLOBALS.write_runstate.send(RunState::ShuttingDown);
                            return Ok(());
                        }
                    };
                    if let Err(e) = self.handle_message(message).await {
                        tracing::error!("{}", e);
                    }
                },
                _ = self.read_runstate.changed() => {
                    match *self.read_runstate.borrow_and_update() {
                        RunState::ShuttingDown => break 'mainloop,

                        // Minions will shut themselves down. Forget about all the jobs.
                        // When we go back online we start fresh.
                        RunState::Offline => {
                            GLOBALS.relay_picker.init().await?;
                            GLOBALS.connected_relays.clear();
                        },
                        _ => { }
                    }
                },
                task_nextjoined = self.minions.join_next_with_id(), if !self.minions.is_empty() => {
                    self.handle_task_nextjoined(task_nextjoined).await;
                },
                v = interrupt_signal.recv() => if v.is_some() {
                    tracing::info!("SIGINT");
                    let _ = GLOBALS.write_runstate.send(RunState::ShuttingDown);
                    break;
                },
                v = quit_signal.recv() => if v.is_some() {
                    tracing::info!("SIGQUIT");
                    let _ = GLOBALS.write_runstate.send(RunState::ShuttingDown);
                    break;
                },
                v = terminate_signal.recv() => if v.is_some() {
                    tracing::info!("SIGTERM");
                    let _ = GLOBALS.write_runstate.send(RunState::ShuttingDown);
                    break;
                },

            }
        }

        Ok(())
    }

    async fn pick_relays(&mut self) {
        // Garbage collect
        match GLOBALS.relay_picker.garbage_collect().await {
            Ok(mut idle) => {
                // Finish those jobs, maybe disconnecting those relays
                for relay_url in idle.drain(..) {
                    if let Err(e) =
                        self.finish_job(relay_url, None, Some(RelayConnectionReason::Follow))
                    {
                        tracing::error!("{}", e);
                        // continue with others
                    }
                }
            }
            Err(e) => {
                tracing::error!("{}", e);
                // continue trying
            }
        };

        loop {
            match GLOBALS.relay_picker.pick().await {
                Err(failure) => {
                    tracing::debug!("Done picking relays: {}", failure);
                    break;
                }
                Ok(relay_url) => {
                    if let Some(ra) = GLOBALS.relay_picker.get_relay_assignment(&relay_url) {
                        tracing::debug!(
                            "Picked {} covering {} pubkeys",
                            &relay_url,
                            ra.pubkeys.len()
                        );
                        // Apply the relay assignment
                        if let Err(e) = self.apply_relay_assignment(ra.to_owned()).await {
                            tracing::error!("{}", e);
                            // On failure, return it
                            GLOBALS.relay_picker.relay_disconnected(&relay_url, 120);
                        }
                    } else {
                        tracing::warn!("Relay Picker just picked {} but it is already no longer part of it's relay assignments!", &relay_url);
                    }
                }
            }
        }
    }

    async fn apply_relay_assignment(&mut self, assignment: RelayAssignment) -> Result<(), Error> {
        let anchor = GLOBALS.feed.current_anchor();

        let mut jobs = vec![RelayJob {
            reason: RelayConnectionReason::Follow,
            payload: ToMinionPayload {
                job_id: rand::random::<u64>(),
                detail: ToMinionPayloadDetail::SubscribeGeneralFeed(
                    assignment.pubkeys.clone(),
                    anchor,
                ),
            },
        }];

        // Until NIP-65 is in widespread use, we should listen to inbox
        // of us on all these relays too
        // Only do this if we aren't already doing it.
        let mut fetch_inbox = true;
        if let Some(jobs) = GLOBALS.connected_relays.get(&assignment.relay_url) {
            for job in &*jobs {
                if job.reason == RelayConnectionReason::FetchInbox {
                    fetch_inbox = false;
                    break;
                }
            }
        }
        if fetch_inbox {
            jobs.push(RelayJob {
                reason: RelayConnectionReason::FetchInbox,
                payload: ToMinionPayload {
                    job_id: rand::random::<u64>(),
                    detail: ToMinionPayloadDetail::SubscribeInbox(anchor),
                },
            });
        }

        // Subscribe to the general feed
        self.engage_minion(assignment.relay_url.clone(), jobs)
            .await?;

        Ok(())
    }

    async fn engage_minion(&mut self, url: RelayUrl, jobs: Vec<RelayJob>) -> Result<(), Error> {
        let relay = GLOBALS.storage.read_or_create_relay(&url, None)?;

        if GLOBALS
            .storage
            .read_setting_relay_connection_requires_approval()
        {
            match relay.allow_connect {
                Some(true) => (),             // fall through
                Some(false) => return Ok(()), // don't connect to this relay
                None => {
                    // Save the engage_minion request and Ask the user
                    GLOBALS.pending.insert(PendingItem::RelayConnectionRequest {
                        relay: url.clone(),
                        jobs: jobs.clone(),
                    });
                    return Ok(());
                }
            }
        } // else fall through

        self.engage_minion_inner(relay, url, jobs).await
    }

    async fn engage_minion_inner(
        &mut self,
        relay: Relay,
        url: RelayUrl,
        mut jobs: Vec<RelayJob>,
    ) -> Result<(), Error> {
        // Do not connect if we are offline
        if GLOBALS.storage.read_setting_offline() {
            return Ok(());
        }

        if jobs.is_empty() {
            return Ok(());
        }

        // don't connect while avoiding this relay
        if relay.should_avoid() {
            return Ok(());
        }

        // don't connect to rank=0 relays
        if relay.rank == 0 {
            return Ok(());
        }

        let entry = GLOBALS.connected_relays.entry(url.clone());

        if let Entry::Occupied(mut oe) = entry {
            // We are already connected. Send it the jobs
            for job in jobs.drain(..) {
                let _ = self.to_minions.send(ToMinionMessage {
                    target: url.as_str().to_owned(),
                    payload: job.payload.clone(),
                });

                let vec = oe.get_mut();

                // Record the job:
                // If the relay already has a job of the same RelayConnectionReason
                // and that reason is not persistent, then this job replaces that
                // one (e.g. FetchAugments)
                if !job.reason.persistent() {
                    if let Some(pos) = vec.iter().position(|e| e.reason == job.reason) {
                        vec[pos] = job;
                        return Ok(());
                    }
                }
                vec.push(job);
            }
        } else {
            // Start up the minion
            let mut minion = Minion::new(url.clone()).await?;
            let payloads = jobs.iter().map(|job| job.payload.clone()).collect();
            let abort_handle = self
                .minions
                .spawn(async move { minion.handle(payloads).await });
            let id = abort_handle.id();
            self.minions_task_url.insert(id, url.clone());

            entry.insert(jobs);
        }

        Ok(())
    }

    async fn handle_task_nextjoined(
        &mut self,
        task_nextjoined: Option<Result<(task::Id, MinionResult), task::JoinError>>,
    ) {
        if task_nextjoined.is_none() {
            return; // rare but possible
        }

        let join_result = task_nextjoined.unwrap();
        let id = match join_result {
            Err(ref join_error) => join_error.id(),
            Ok((id, _)) => id,
        };
        let url = match self.minions_task_url.get(&id).cloned() {
            Some(url) => url,
            None => return, // unknown minion!
        };

        // Remove from our hashmap
        self.minions_task_url.remove(&id);

        // Set to not connected, and take any unfinished jobs
        let mut relayjobs = GLOBALS
            .connected_relays
            .remove(&url)
            .map(|(_, v)| v)
            .unwrap_or_default();

        // Exclusion will be non-zero if there was a failure.  It will be zero if we
        // succeeded
        let mut exclusion: u64;

        match join_result {
            Err(join_error) => {
                tracing::error!("Minion {} completed with join error: {}", &url, join_error);
                Self::bump_failure_count(&url);
                exclusion = 60 * 2;
            }
            Ok((_id, result)) => match result {
                Ok(exitreason) => {
                    if exitreason.benign() {
                        tracing::debug!("Minion {} completed: {:?}", &url, exitreason);
                    } else {
                        tracing::info!("Minion {} completed: {:?}", &url, exitreason);
                    }
                    exclusion = match exitreason {
                        MinionExitReason::GotDisconnected => 60 * 2,
                        MinionExitReason::GotShutdownMessage => 0,
                        MinionExitReason::GotWSClose => 60 * 2,
                        MinionExitReason::LostOverlord => 0,
                        MinionExitReason::SubscriptionsCompletedSuccessfully => {
                            // The jobs completed but we didn't get messages for them before the
                            // minion exited. Clear those jobs.
                            relayjobs = vec![];
                            0
                        }
                        MinionExitReason::SubscriptionsCompletedWithFailures => 60 * 2,
                        MinionExitReason::Unknown => 60 * 2,
                    };
                }
                Err(e) => {
                    Self::bump_failure_count(&url);
                    tracing::error!("Minion {} completed with error: {}", &url, e);
                    exclusion = 60 * 2;
                    if let ErrorKind::RelayRejectedUs = e.kind {
                        exclusion = 60 * 10;
                    } else if let ErrorKind::ReqwestHttpError(_) = e.kind {
                        exclusion = 60 * 10;
                    } else if let ErrorKind::Timeout(_) = e.kind {
                        exclusion = 60; // could be local issue affecting all relays so cannot go too big.
                    } else if let ErrorKind::Websocket(wserror) = e.kind {
                        if let tungstenite::error::Error::Http(response) = wserror {
                            exclusion = match response.status() {
                                StatusCode::MOVED_PERMANENTLY => 60 * 10,
                                StatusCode::PERMANENT_REDIRECT => 60 * 10,
                                StatusCode::UNAUTHORIZED => 60 * 10,
                                StatusCode::PAYMENT_REQUIRED => 60 * 10,
                                StatusCode::FORBIDDEN => 60 * 10,
                                StatusCode::NOT_FOUND => 60 * 10,
                                StatusCode::PROXY_AUTHENTICATION_REQUIRED => 60 * 10,
                                StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS => 60 * 10,
                                StatusCode::NOT_IMPLEMENTED => 60 * 10,
                                StatusCode::BAD_GATEWAY => 60 * 10,
                                StatusCode::SERVICE_UNAVAILABLE => 60 * 10,
                                s if s.as_u16() >= 500 => 60 * 10,
                                s if s.as_u16() >= 400 => 60 * 2,
                                _ => 60 * 2,
                            };
                        } else if let tungstenite::error::Error::ConnectionClosed = wserror {
                            tracing::debug!("Minion {} completed", &url);
                            exclusion = 15; // was not actually an error, but needs a pause
                        } else if let tungstenite::error::Error::Protocol(protocol_error) = wserror
                        {
                            exclusion = match protocol_error {
                                tungstenite::error::ProtocolError::ResetWithoutClosingHandshake => {
                                    60
                                }
                                _ => 60 * 2,
                            }
                        } else {
                            let f = format!("{}", wserror);
                            if f.contains("failed to lookup address")
                                || f.contains("No route to host")
                            {
                                exclusion = 60; // could be local issue affecting all relays so cannot go too big.
                            }
                        }
                    }
                }
            },
        };

        // Act upon this minion exiting, unless we are quitting
        if self.read_runstate.borrow().going_online() {
            self.recover_from_minion_exit(url, relayjobs, exclusion)
                .await;
        }
    }

    async fn recover_from_minion_exit(
        &mut self,
        url: RelayUrl,
        jobs: Vec<RelayJob>,
        mut exclusion: u64,
    ) {
        // Randomize the exclusion to between half and full
        use rand::Rng;
        if exclusion > 1 {
            exclusion = rand::thread_rng()
                .sample(rand::distributions::Uniform::new(exclusion / 2, exclusion));
        }

        // Let the relay picker know it disconnected
        GLOBALS
            .relay_picker
            .relay_disconnected(&url, exclusion as i64);

        // For people we are following, pick relays
        if let Err(e) = GLOBALS.relay_picker.refresh_person_relay_scores().await {
            tracing::error!("Error: {}", e);
        }
        self.pick_relays().await;

        if exclusion == 0 {
            return;
        }

        if jobs.is_empty() {
            return;
        }

        // Record the exclusion in the relay record
        if let Ok(Some(mut relay)) = GLOBALS.storage.read_relay(&url, None) {
            let until = Unixtime::now() + Duration::from_secs(exclusion);
            relay.avoid_until = Some(until);
            let _ = GLOBALS.storage.write_relay(&relay, None);
        }

        // If none of the jobs were persistent, we are done
        if !jobs.iter().any(|j| j.reason.persistent()) {
            return;
        }

        // We have unfinished persistent jobs.  We need to restart this relay after
        // the exclusion (as long as it is reasonably short)

        // safety catch, minimum exclusion is 10s
        let exclusion = exclusion.max(10);

        tracing::info!(
            "Minion {} will restart in {} seconds to continue persistent jobs",
            &url,
            exclusion
        );

        std::mem::drop(tokio::spawn(async move {
            tokio::time::sleep(Duration::new(exclusion, 0)).await;
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::ReengageMinion(url, jobs));
        }));
    }

    async fn reengage_minion(&mut self, url: RelayUrl, jobs: Vec<RelayJob>) -> Result<(), Error> {
        self.engage_minion(url, jobs).await?;

        Ok(())
    }

    fn bump_failure_count(url: &RelayUrl) {
        if let Ok(Some(mut relay)) = GLOBALS.storage.read_relay(url, None) {
            relay.failure_count += 1;
            let _ = GLOBALS.storage.write_relay(&relay, None);
        }
    }

    async fn handle_message(&mut self, message: ToOverlordMessage) -> Result<(), Error> {
        match message {
            ToOverlordMessage::AddRelay(relay_url) => {
                self.add_relay(relay_url).await?;
            }
            ToOverlordMessage::AdvertiseRelayList => {
                self.advertise_relay_list().await?;
            }
            ToOverlordMessage::AdvertiseRelayListOne(relay_url, event, dmevent) => {
                self.advertise_relay_list_one(relay_url, event, dmevent)
                    .await?;
            }
            ToOverlordMessage::AuthApproved(relay_url, permanent) => {
                self.auth_approved(relay_url, permanent)?;
            }
            ToOverlordMessage::AuthDeclined(relay_url, permanent) => {
                self.auth_declined(relay_url, permanent)?;
            }
            ToOverlordMessage::BookmarkAdd(er, private) => {
                self.bookmark_add(er, private).await?;
            }
            ToOverlordMessage::BookmarkRm(er) => {
                self.bookmark_rm(er).await?;
            }
            ToOverlordMessage::ChangePassphrase { old, new } => {
                Self::change_passphrase(old, new).await?;
            }
            ToOverlordMessage::ClearPersonList(list) => {
                self.clear_person_list(list)?;
            }
            ToOverlordMessage::ConnectApproved(relay_url, permanent) => {
                self.connect_approved(relay_url, permanent).await?;
            }
            ToOverlordMessage::ConnectDeclined(relay_url, permanent) => {
                self.connect_declined(relay_url, permanent).await?;
            }
            ToOverlordMessage::DelegationReset => {
                Self::delegation_reset().await?;
            }
            ToOverlordMessage::DeletePersonList(list) => {
                self.delete_person_list(list).await?;
            }
            ToOverlordMessage::DeletePost(id) => {
                self.delete_post(id).await?;
            }
            ToOverlordMessage::DeletePriv => {
                Self::delete_priv().await?;
            }
            ToOverlordMessage::DeletePub => {
                Self::delete_pub().await?;
            }
            ToOverlordMessage::DropRelay(relay_url) => {
                self.drop_relay(relay_url)?;
            }
            ToOverlordMessage::FetchEvent(id, relay_urls) => {
                self.fetch_event(id, relay_urls).await?;
            }
            ToOverlordMessage::FetchEventAddr(ea) => {
                self.fetch_event_addr(ea).await?;
            }
            ToOverlordMessage::FollowPubkey(pubkey, list, private) => {
                self.follow_pubkey(pubkey, list, private).await?;
            }
            ToOverlordMessage::FollowNip05(nip05, list, private) => {
                Self::follow_nip05(nip05, list, private).await?;
            }
            ToOverlordMessage::FollowNprofile(nprofile, list, private) => {
                self.follow_nprofile(nprofile, list, private).await?;
            }
            ToOverlordMessage::GeneratePrivateKey(password) => {
                Self::generate_private_key(password).await?;
            }
            ToOverlordMessage::HideOrShowRelay(relay_url, hidden) => {
                Self::hide_or_show_relay(relay_url, hidden)?;
            }
            ToOverlordMessage::ImportPriv { privkey, password } => {
                Self::import_priv(privkey, password).await?;
            }
            ToOverlordMessage::ImportPub(pubstr) => {
                Self::import_pub(pubstr).await?;
            }
            ToOverlordMessage::Like(id, pubkey) => {
                self.like(id, pubkey).await?;
            }
            ToOverlordMessage::LoadMoreCurrentFeed => {
                self.load_more().await?;
            }
            ToOverlordMessage::MinionJobComplete(url, job_id) => {
                self.finish_job(url, Some(job_id), None)?;
            }
            ToOverlordMessage::MinionJobUpdated(url, old_job_id, new_job_id) => {
                // internal
                if old_job_id != 0 && new_job_id != 0 {
                    if let Some(mut refmut) = GLOBALS.connected_relays.get_mut(&url) {
                        refmut.value_mut().retain_mut(|job| {
                            if job.payload.job_id == new_job_id {
                                false // remove the new job
                            } else if job.payload.job_id == old_job_id {
                                job.payload.job_id = new_job_id;
                                true // keep the old job, with modified job id
                            } else {
                                true // keep the rest
                            }
                        });
                    }
                }
            }
            ToOverlordMessage::Nip46ServerOpApprovalResponse(pubkey, parsed_command, approval) => {
                self.nip46_server_op_approval_response(pubkey, parsed_command, approval)
                    .await?;
            }
            ToOverlordMessage::RefreshScoresAndPickRelays => {
                self.refresh_scores_and_pick_relays().await?;
            }
            ToOverlordMessage::Post {
                content,
                tags,
                in_reply_to,
                annotation,
                dm_channel,
            } => {
                self.post(content, tags, in_reply_to, annotation, dm_channel)
                    .await?;
            }
            ToOverlordMessage::PostAgain(event) => {
                self.post_again(event).await?;
            }
            ToOverlordMessage::PostNip46Event(event, relays) => {
                self.post_nip46_event(event, relays).await?;
            }
            ToOverlordMessage::PruneCache => {
                Self::prune_cache().await?;
            }
            ToOverlordMessage::PruneDatabase => {
                Self::prune_database()?;
            }
            ToOverlordMessage::PushPersonList(person_list) => {
                self.push_person_list(person_list).await?;
            }
            ToOverlordMessage::PushMetadata(metadata) => {
                self.push_metadata(metadata).await?;
            }
            ToOverlordMessage::RankRelay(relay_url, rank) => {
                Self::rank_relay(relay_url, rank)?;
            }
            ToOverlordMessage::ReengageMinion(url, jobs) => {
                self.reengage_minion(url, jobs).await?;
            }
            ToOverlordMessage::RefreshSubscribedMetadata => {
                self.refresh_subscribed_metadata().await?;
            }
            ToOverlordMessage::Repost(id) => {
                self.repost(id).await?;
            }
            ToOverlordMessage::Search(text) => {
                Self::search(text).await?;
            }
            ToOverlordMessage::SetActivePerson(pubkey) => {
                Self::set_active_person(pubkey).await?;
            }
            ToOverlordMessage::SetDmChannel(dmchannel) => {
                self.set_dm_channel(dmchannel).await?;
            }
            ToOverlordMessage::SetPersonFeed(pubkey, anchor) => {
                self.set_person_feed(pubkey, anchor).await?;
            }
            ToOverlordMessage::SetThreadFeed {
                id,
                referenced_by,
                author,
            } => {
                self.set_thread_feed(id, referenced_by, author).await?;
            }
            ToOverlordMessage::StartLongLivedSubscriptions => {
                self.start_long_lived_subscriptions().await?;
            }
            ToOverlordMessage::SubscribeConfig(opt_relays) => {
                self.subscribe_config(opt_relays).await?;
            }
            ToOverlordMessage::SubscribeDiscover(pubkeys, opt_relays) => {
                self.subscribe_discover(pubkeys, opt_relays).await?;
            }
            ToOverlordMessage::SubscribeInbox(opt_relays) => {
                self.subscribe_inbox(opt_relays).await?;
            }
            ToOverlordMessage::SubscribeNip46(relays) => {
                self.subscribe_nip46(relays).await?;
            }
            ToOverlordMessage::UnlockKey(password) => {
                Self::unlock_key(password)?;
            }
            ToOverlordMessage::UpdateMetadata(pubkey) => {
                self.update_metadata(pubkey).await?;
            }
            ToOverlordMessage::UpdateMetadataInBulk(pubkeys) => {
                self.update_metadata_in_bulk(pubkeys).await?;
            }
            ToOverlordMessage::UpdatePersonList { person_list, merge } => {
                self.update_person_list(person_list, merge).await?;
            }
            ToOverlordMessage::UpdateRelay(old, new) => {
                self.update_relay(old, new).await?;
            }
            ToOverlordMessage::VisibleNotesChanged(visible) => {
                self.visible_notes_changed(visible).await?;
            }
            ToOverlordMessage::ZapStart(id, pubkey, lnurl) => {
                self.zap_start(id, pubkey, lnurl).await?;
            }
            ToOverlordMessage::Zap(id, pubkey, msats, comment) => {
                self.zap(id, pubkey, msats, comment).await?;
            }
        }

        Ok(())
    }

    /// Add a new relay to gossip
    pub async fn add_relay(&mut self, relay_url: RelayUrl) -> Result<(), Error> {
        // Create relay if missing
        GLOBALS.storage.write_relay_if_missing(&relay_url, None)?;

        // Then pick relays again (possibly including the one added)
        GLOBALS.relay_picker.refresh_person_relay_scores().await?;
        self.pick_relays().await;

        Ok(())
    }

    /// Advertise the user's current relay list
    pub async fn advertise_relay_list(&mut self) -> Result<(), Error> {
        let public_key = match GLOBALS.identity.public_key() {
            Some(pk) => pk,
            None => {
                tracing::warn!("No public key! Not posting");
                return Ok(());
            }
        };

        let event = {
            let inbox_or_outbox_relays: Vec<Relay> = GLOBALS.storage.filter_relays(|r| {
                r.has_usage_bits(Relay::INBOX) || r.has_usage_bits(Relay::OUTBOX)
            })?;
            let mut tags: Vec<Tag> = Vec::new();
            for relay in inbox_or_outbox_relays.iter() {
                let marker =
                    if relay.has_usage_bits(Relay::INBOX) && relay.has_usage_bits(Relay::OUTBOX) {
                        None
                    } else if relay.has_usage_bits(Relay::INBOX) {
                        Some("read".to_owned()) // NIP-65 uses the term 'read' instead of 'inbox'
                    } else if relay.has_usage_bits(Relay::OUTBOX) {
                        Some("write".to_owned()) // NIP-65 uses the term 'write' instead of 'outbox'
                    } else {
                        unreachable!()
                    };

                tags.push(Tag::new_relay(relay.url.to_unchecked_url(), marker));
            }

            let pre_event = PreEvent {
                pubkey: public_key,
                created_at: Unixtime::now(),
                kind: EventKind::RelayList,
                tags,
                content: "".to_string(),
            };

            GLOBALS.identity.sign_event(pre_event)?
        };

        let dmevent = {
            let dm_relays: Vec<Relay> = GLOBALS
                .storage
                .filter_relays(|r| r.has_usage_bits(Relay::DM))?;
            let mut tags: Vec<Tag> = Vec::new();
            for relay in dm_relays.iter() {
                tags.push(Tag::new(&["relay", relay.url.as_str()]));
            }

            let pre_event = PreEvent {
                pubkey: public_key,
                created_at: Unixtime::now(),
                kind: EventKind::DmRelayList,
                tags,
                content: "".to_string(),
            };

            GLOBALS.identity.sign_event(pre_event)?
        };

        let mut relays = Relay::choose_relays(0, |r| r.is_good_for_advertise())?;
        relays.sort_by(|a, b| {
            a.rank.cmp(&b.rank).then(
                a.success_rate()
                    .partial_cmp(&b.success_rate())
                    .unwrap()
                    .then(a.success_count.cmp(&b.success_count)),
            )
        });

        let _ = GLOBALS
            .advertise_jobs_remaining
            .fetch_add(relays.len(), Ordering::SeqCst);

        std::mem::drop(tokio::spawn(async move {
            for relay in relays.drain(..) {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::AdvertiseRelayListOne(
                        relay.url.clone(),
                        Box::new(event.clone()),
                        Box::new(dmevent.clone()),
                    ));

                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }));

        Ok(())
    }

    /// Advertise the user's current relay list to one relay
    pub async fn advertise_relay_list_one(
        &mut self,
        relay_url: RelayUrl,
        event: Box<Event>,
        dmevent: Box<Event>,
    ) -> Result<(), Error> {
        let job_id = rand::random::<u64>();

        // Send it the event to post
        tracing::debug!("Asking {} to advertise relay list", &relay_url);
        if let Err(e) = self
            .engage_minion(
                relay_url,
                vec![RelayJob {
                    reason: RelayConnectionReason::Advertising,
                    payload: ToMinionPayload {
                        job_id,
                        detail: ToMinionPayloadDetail::AdvertiseRelayList(event, dmevent),
                    },
                }],
            )
            .await
        {
            tracing::error!("{}", e);
        }

        let _ = GLOBALS
            .advertise_jobs_remaining
            .fetch_sub(1, Ordering::SeqCst);

        Ok(())
    }

    /// User has approved authentication on this relay. Save this result for later
    /// and inform the minion.
    pub fn auth_approved(&mut self, relay_url: RelayUrl, permanent: bool) -> Result<(), Error> {
        if permanent {
            // Save the answer in the relay record
            GLOBALS.storage.modify_relay(
                &relay_url,
                |r| {
                    r.allow_auth = Some(true);
                },
                None,
            )?;
        }

        if GLOBALS.connected_relays.contains_key(&relay_url) {
            // Tell the minion
            let _ = self.to_minions.send(ToMinionMessage {
                target: relay_url.as_str().to_owned(),
                payload: ToMinionPayload {
                    job_id: 0,
                    detail: ToMinionPayloadDetail::AuthApproved,
                },
            });
        } else {
            // Clear the auth request, we are no longer connected
            if let Some(pubkey) = GLOBALS.identity.public_key() {
                GLOBALS
                    .pending
                    .take_relay_authentication_request(&pubkey, &relay_url);
            }
        }

        Ok(())
    }

    /// User has declined authentication on this relay. Save this result for later
    /// and inform the minion.
    pub fn auth_declined(&mut self, relay_url: RelayUrl, permanent: bool) -> Result<(), Error> {
        if permanent {
            // Save the answer in the relay record
            GLOBALS.storage.modify_relay(
                &relay_url,
                |r| {
                    r.allow_auth = Some(false);
                },
                None,
            )?;
        }

        if GLOBALS.connected_relays.contains_key(&relay_url) {
            // Tell the minion
            let _ = self.to_minions.send(ToMinionMessage {
                target: relay_url.as_str().to_owned(),
                payload: ToMinionPayload {
                    job_id: 0,
                    detail: ToMinionPayloadDetail::AuthDeclined,
                },
            });
        } else {
            // Clear the auth request, we are no longer connected
            if let Some(pubkey) = GLOBALS.identity.public_key() {
                GLOBALS
                    .pending
                    .take_relay_authentication_request(&pubkey, &relay_url);
            }
        }

        Ok(())
    }

    async fn post_bookmarks(&mut self, event: Event) -> Result<(), Error> {
        // Process this event locally (ignore any error)
        let _ = crate::process::process_new_event(&event, None, None, false, false);

        let config_relays: Vec<RelayUrl> = Relay::choose_relay_urls(Relay::WRITE, |_| true)?;

        for relay_url in config_relays.iter() {
            self.engage_minion(
                relay_url.to_owned(),
                vec![RelayJob {
                    reason: RelayConnectionReason::PostEvent,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvents(vec![event.clone()]),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    /// Adds or removes a bookmark, and publishes new bookmarks list
    pub async fn bookmark_add(&mut self, er: EventReference, private: bool) -> Result<(), Error> {
        let added = GLOBALS.bookmarks.write().add(er, private)?;

        if added {
            GLOBALS.recompute_current_bookmarks.notify_one();
            let event = GLOBALS.bookmarks.read().into_event()?;
            self.post_bookmarks(event).await?;
        }

        Ok(())
    }

    /// Adds or removes a bookmark, and publishes new bookmarks list
    pub async fn bookmark_rm(&mut self, er: EventReference) -> Result<(), Error> {
        let removed = GLOBALS.bookmarks.write().remove(er)?;

        if removed {
            GLOBALS.recompute_current_bookmarks.notify_one();
            let event = GLOBALS.bookmarks.read().into_event()?;
            self.post_bookmarks(event).await?;
        }

        Ok(())
    }

    /// Change the user's passphrase.
    pub async fn change_passphrase(mut old: String, mut new: String) -> Result<(), Error> {
        GLOBALS.identity.change_passphrase(&old, &new).await?;
        old.zeroize();
        new.zeroize();
        Ok(())
    }

    /// Clear the specified person lit. This wipes everybody. But it doesn't publish
    /// the empty list. You should probably double-check that the user is certain.
    pub fn clear_person_list(&mut self, list: PersonList) -> Result<(), Error> {
        GLOBALS.people.clear_person_list(list)?;
        Ok(())
    }

    /// User has approved connection to this relay. Save this result for later
    /// and inform the minion.
    pub async fn connect_approved(
        &mut self,
        relay_url: RelayUrl,
        permanent: bool,
    ) -> Result<(), Error> {
        if permanent {
            // Save the answer in the relay record
            GLOBALS.storage.modify_relay(
                &relay_url,
                |r| {
                    r.allow_connect = Some(true);
                },
                None,
            )?;
        }

        // Start the job
        if let Some((url, jobs)) = GLOBALS.pending.take_relay_connection_request(&relay_url) {
            let relay = GLOBALS.storage.read_or_create_relay(&url, None)?;
            self.engage_minion_inner(relay, url, jobs).await?;
        }

        Ok(())
    }

    /// User has declined connection to this relay. Save this result for later
    /// and inform the minion.
    pub async fn connect_declined(
        &mut self,
        relay_url: RelayUrl,
        permanent: bool,
    ) -> Result<(), Error> {
        if permanent {
            // Save the answer in the relay record
            GLOBALS.storage.modify_relay(
                &relay_url,
                |r| {
                    r.allow_connect = Some(false);
                },
                None,
            )?;
        }

        // Remove the connect requests entry
        GLOBALS.pending.take_relay_connection_request(&relay_url);

        Ok(())
    }

    /// Remove any key delegation setup
    pub async fn delegation_reset() -> Result<(), Error> {
        if GLOBALS.delegation.reset() {
            // save and statusmsg
            GLOBALS.delegation.save().await?;
            GLOBALS
                .status_queue
                .write()
                .write("Delegation tag removed".to_string());
        }
        Ok(())
    }

    /// Delete a person list
    pub async fn delete_person_list(&mut self, list: PersonList) -> Result<(), Error> {
        // Get the metadata first, we need it to delete events
        let metadata = match GLOBALS.storage.get_person_list_metadata(list)? {
            Some(m) => m,
            None => return Ok(()),
        };

        // Delete the list locally
        let mut txn = GLOBALS.storage.get_write_txn()?;
        GLOBALS.storage.clear_person_list(list, Some(&mut txn))?;
        GLOBALS
            .storage
            .deallocate_person_list(list, Some(&mut txn))?;
        txn.commit()?;

        // If we are only following, nothing else needed
        if GLOBALS.storage.get_flag_following_only() {
            return Ok(());
        }

        let public_key = match GLOBALS.identity.public_key() {
            Some(pk) => pk,
            None => {
                // Odd. how do they have a list if they have no pubkey?
                return Ok(());
            }
        };

        let mut filter = Filter::new();
        filter.add_event_kind(EventKind::FollowSets);
        filter.add_author(&public_key.into());

        // Find all local-storage events that define the list
        let bad_events = GLOBALS.storage.find_events_by_filter(&filter, |event| {
            event.parameter().as_ref() == Some(&metadata.dtag)
        })?;

        // If no list events, we are done
        if bad_events.is_empty() {
            return Ok(());
        }

        // Delete those events locally
        for bad_event in &bad_events {
            GLOBALS.storage.delete_event(bad_event.id, None)?;
        }

        // Require sign in to delete further
        if !GLOBALS.identity.is_unlocked() {
            GLOBALS
                .status_queue
                .write()
                .write("The list was only deleted locally because you are not signed in. The list may reappear on restart.".to_string());
            return Ok(());
        }

        // Generate a deletion event for those events
        let event = {
            // Include an "a" tag for the entire group
            let ea = EventAddr {
                d: metadata.dtag.clone(),
                relays: vec![],
                kind: EventKind::FollowSets,
                author: public_key,
            };
            let mut tags: Vec<Tag> = vec![Tag::new_address(&ea, None)];

            // Include "e" tags for each event
            for bad_event in &bad_events {
                tags.push(Tag::new_event(bad_event.id, None, None));
            }

            let pre_event = PreEvent {
                pubkey: public_key,
                created_at: Unixtime::now(),
                kind: EventKind::EventDeletion,
                tags,
                content: "Deleting person list".to_owned(),
            };

            // Should we add a pow? Maybe the relay needs it.
            GLOBALS.identity.sign_event(pre_event)?
        };

        // Process this event locally
        crate::process::process_new_event(&event, None, None, false, false)?;

        // Determine which relays to post this to
        let mut relay_urls: Vec<RelayUrl> = Vec::new();
        {
            // Get all of the relays that we write to
            let write_relays = Relay::choose_relay_urls(Relay::WRITE, |_| true)?;
            relay_urls.extend(write_relays);

            // Get all of the relays this events were seen on
            for bad_event in &bad_events {
                let seen_on: Vec<RelayUrl> = GLOBALS
                    .storage
                    .get_event_seen_on_relay(bad_event.id)?
                    .iter()
                    .take(6) // Doesn't have to be everywhere
                    .map(|(url, _time)| url.to_owned())
                    .collect();

                for url in &seen_on {
                    tracing::error!("SEEN ON {}", &url);
                }

                relay_urls.extend(seen_on);
            }

            relay_urls.sort();
            relay_urls.dedup();
        }

        // Send event to all these relays
        for url in relay_urls {
            self.engage_minion(
                url.to_owned(),
                vec![RelayJob {
                    reason: RelayConnectionReason::PostEvent,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvents(vec![event.clone()]),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    /// Delete a post
    pub async fn delete_post(&mut self, id: Id) -> Result<(), Error> {
        let mut tags: Vec<Tag> = vec![Tag::new_event(id, None, None)];

        if let Some(target_event) = GLOBALS.storage.read_event(id)? {
            tags.push(Tag::new_kind(target_event.kind));
        }

        let event = {
            let public_key = match GLOBALS.identity.public_key() {
                Some(pk) => pk,
                None => {
                    tracing::warn!("No public key! Not posting");
                    return Ok(());
                }
            };

            let pre_event = PreEvent {
                pubkey: public_key,
                created_at: Unixtime::now(),
                kind: EventKind::EventDeletion,
                tags,
                content: "".to_owned(), // FIXME, option to supply a delete reason
            };

            // Should we add a pow? Maybe the relay needs it.
            GLOBALS.identity.sign_event(pre_event)?
        };

        // Process this event locally
        crate::process::process_new_event(&event, None, None, false, false)?;

        // Determine which relays to post this to
        let mut relay_urls: Vec<RelayUrl> = Vec::new();
        {
            // Get all of the relays that we write to
            let write_relays = Relay::choose_relay_urls(Relay::WRITE, |_| true)?;
            relay_urls.extend(write_relays);

            // Get all of the relays this event was seen on
            let seen_on: Vec<RelayUrl> = GLOBALS
                .storage
                .get_event_seen_on_relay(id)?
                .iter()
                .take(6) // doesn't have to be everywhere
                .map(|(url, _time)| url.to_owned())
                .collect();
            relay_urls.extend(seen_on);

            relay_urls.sort();
            relay_urls.dedup();
        }

        for url in relay_urls {
            // Send it the event to post
            tracing::debug!("Asking {} to delete", &url);

            self.engage_minion(
                url.to_owned(),
                vec![RelayJob {
                    reason: RelayConnectionReason::PostEvent,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvents(vec![event.clone()]),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    /// Delete private key and any delegation setup
    pub async fn delete_priv() -> Result<(), Error> {
        GLOBALS.identity.delete_identity()?;
        Self::delegation_reset().await?;
        GLOBALS
            .status_queue
            .write()
            .write("Identity deleted.".to_string());
        Ok(())
    }

    /// Delete public key (only if no private key exists) and any delegation setup
    pub async fn delete_pub() -> Result<(), Error> {
        GLOBALS.identity.clear_public_key()?;
        Self::delegation_reset().await?;
        Ok(())
    }

    /// Disconnect from the specified relay. This may not happen immediately if the minion
    /// handling that relay is stuck waiting for a timeout.
    pub fn drop_relay(&mut self, relay_url: RelayUrl) -> Result<(), Error> {
        let _ = self.to_minions.send(ToMinionMessage {
            target: relay_url.as_str().to_owned(),
            payload: ToMinionPayload {
                job_id: 0,
                detail: ToMinionPayloadDetail::Shutdown,
            },
        });

        Ok(())
    }

    /// Fetch an event from a specific relay by event `Id`
    pub async fn fetch_event(
        &mut self,
        id: Id,
        mut relay_urls: Vec<RelayUrl>,
    ) -> Result<(), Error> {
        // Use READ relays if relays are unknown
        if relay_urls.is_empty() {
            relay_urls = Relay::choose_relay_urls(Relay::READ, |_| true)?;
        }

        // Don't do this if we already have the event
        if !GLOBALS.storage.has_event(id)? {
            // Note: minions will remember if they get the same id multiple times
            //       not to fetch it multiple times.

            for url in relay_urls.iter() {
                self.engage_minion(
                    url.to_owned(),
                    vec![RelayJob {
                        reason: RelayConnectionReason::FetchEvent,
                        payload: ToMinionPayload {
                            job_id: rand::random::<u64>(),
                            detail: ToMinionPayloadDetail::FetchEvent(id),
                        },
                    }],
                )
                .await?;
            }
        }

        Ok(())
    }

    /// Fetch an event based on an `EventAddr`
    pub async fn fetch_event_addr(&mut self, ea: EventAddr) -> Result<(), Error> {
        for unchecked_url in ea.relays.iter() {
            if let Ok(relay_url) = RelayUrl::try_from_unchecked_url(unchecked_url) {
                self.engage_minion(
                    relay_url.to_owned(),
                    vec![RelayJob {
                        reason: RelayConnectionReason::FetchEvent,
                        payload: ToMinionPayload {
                            job_id: rand::random::<u64>(),
                            detail: ToMinionPayloadDetail::FetchEventAddr(ea.clone()),
                        },
                    }],
                )
                .await?;
            }
        }

        Ok(())
    }

    /// Follow a person by `PublicKey`
    pub async fn follow_pubkey(
        &mut self,
        pubkey: PublicKey,
        list: PersonList,
        private: Private,
    ) -> Result<(), Error> {
        GLOBALS.people.follow(&pubkey, true, list, private)?;
        tracing::debug!("Followed {}", &pubkey.as_hex_string());
        Ok(())
    }

    /// Follow a person by a nip-05 address
    pub async fn follow_nip05(
        nip05: String,
        list: PersonList,
        private: Private,
    ) -> Result<(), Error> {
        std::mem::drop(tokio::spawn(async move {
            if let Err(e) = crate::nip05::get_and_follow_nip05(nip05, list, private).await {
                tracing::error!("{}", e);
            }
        }));
        Ok(())
    }

    /// Follow a person by a `Profile` (nprofile1...)
    pub async fn follow_nprofile(
        &mut self,
        nprofile: Profile,
        list: PersonList,
        private: Private,
    ) -> Result<(), Error> {
        // Set their relays
        for relay in nprofile.relays.iter() {
            if let Ok(relay_url) = RelayUrl::try_from_unchecked_url(relay) {
                // Create relay if missing
                GLOBALS.storage.write_relay_if_missing(&relay_url, None)?;

                // Save person_relay
                GLOBALS.storage.modify_person_relay(
                    nprofile.pubkey,
                    &relay_url,
                    |pr| {
                        pr.last_suggested = Some(Unixtime::now().0 as u64);
                    },
                    None,
                )?
            }
        }

        // Follow
        GLOBALS
            .people
            .follow(&nprofile.pubkey, true, list, private)?;

        GLOBALS
            .status_queue
            .write()
            .write(format!("Followed user at {} relays", nprofile.relays.len()));

        Ok(())
    }

    /// Generate an identity (private key) and keep encrypted under the given passphrase
    pub async fn generate_private_key(mut password: String) -> Result<(), Error> {
        GLOBALS.identity.generate_private_key(&password)?;
        password.zeroize();
        Ok(())
    }

    /// Hide or Show a relay. This adjusts the `hidden` a flag on the `Relay` record
    /// (You could easily do this yourself by talking to GLOBALS.storage directly too)
    pub fn hide_or_show_relay(relay_url: RelayUrl, hidden: bool) -> Result<(), Error> {
        if let Some(mut relay) = GLOBALS.storage.read_relay(&relay_url, None)? {
            relay.hidden = hidden;
            GLOBALS.storage.write_relay(&relay, None)?;
        }

        Ok(())
    }

    /// Import a private key
    pub async fn import_priv(mut privkey: String, mut password: String) -> Result<(), Error> {
        if privkey.starts_with("ncryptsec") {
            let epk = EncryptedPrivateKey(privkey);
            match GLOBALS.identity.set_encrypted_private_key(epk, &password) {
                Ok(_) => {
                    GLOBALS.identity.unlock(&password)?;
                    password.zeroize();
                }
                Err(err) => {
                    password.zeroize();
                    GLOBALS
                        .status_queue
                        .write()
                        .write(format!("Error importing ncryptsec: {}", err));
                }
            }
        } else {
            let maybe_pk1 = PrivateKey::try_from_bech32_string(privkey.trim());
            let maybe_pk2 = PrivateKey::try_from_hex_string(privkey.trim());
            privkey.zeroize();
            if maybe_pk1.is_err() && maybe_pk2.is_err() {
                password.zeroize();
                GLOBALS
                    .status_queue
                    .write()
                    .write("Private key not recognized.".to_owned());
            } else {
                let privkey = maybe_pk1.unwrap_or_else(|_| maybe_pk2.unwrap());
                GLOBALS.identity.set_private_key(privkey, &password)?;
                password.zeroize();
            }
        }

        Ok(())
    }

    /// Import a public key only (npub or hex)
    pub async fn import_pub(pubstr: String) -> Result<(), Error> {
        let maybe_pk1 = PublicKey::try_from_bech32_string(pubstr.trim(), true);
        let maybe_pk2 = PublicKey::try_from_hex_string(pubstr.trim(), true);
        if maybe_pk1.is_err() && maybe_pk2.is_err() {
            GLOBALS
                .status_queue
                .write()
                .write("Public key not recognized.".to_owned());
        } else {
            let pubkey = maybe_pk1.unwrap_or_else(|_| maybe_pk2.unwrap());
            GLOBALS.identity.set_public_key(pubkey)?;
        }

        Ok(())
    }

    /// Like a post. The backend doesn't read the event, so you have to supply the
    /// pubkey author too.
    pub async fn like(&mut self, id: Id, pubkey: PublicKey) -> Result<(), Error> {
        let event = {
            let public_key = match GLOBALS.identity.public_key() {
                Some(pk) => pk,
                None => {
                    tracing::warn!("No public key! Not posting");
                    return Ok(());
                }
            };

            let mut tags: Vec<Tag> = vec![
                Tag::new_event(
                    id,
                    relay::recommended_relay_hint(id)?.map(|rr| rr.to_unchecked_url()),
                    None,
                ),
                Tag::new_pubkey(pubkey, None, None),
            ];

            if GLOBALS.storage.read_setting_set_client_tag() {
                tags.push(Tag::new(&["client", "gossip"]));
            }

            let pre_event = PreEvent {
                pubkey: public_key,
                created_at: Unixtime::now(),
                kind: EventKind::Reaction,
                tags,
                content: "+".to_owned(),
            };

            let powint = GLOBALS.storage.read_setting_pow();
            if powint > 0 {
                let (work_sender, work_receiver) = mpsc::channel();
                std::thread::spawn(move || {
                    work_logger(work_receiver, powint);
                });
                GLOBALS
                    .identity
                    .sign_event_with_pow(pre_event, powint, Some(work_sender))?
            } else {
                GLOBALS.identity.sign_event(pre_event)?
            }
        };

        let relays: Vec<Relay> = Relay::choose_relays(Relay::WRITE, |_| true)?;
        // FIXME - post it to relays we have seen it on.

        for relay in relays {
            // Send it the event to post
            tracing::debug!("Asking {} to post", &relay.url);

            self.engage_minion(
                relay.url.clone(),
                vec![RelayJob {
                    reason: RelayConnectionReason::PostLike,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvents(vec![event.clone()]),
                    },
                }],
            )
            .await?;
        }

        // Process the message for ourself
        crate::process::process_new_event(&event, None, None, false, false)?;

        Ok(())
    }

    pub async fn load_more(&mut self) -> Result<(), Error> {
        // Change the feed range:
        let anchor = GLOBALS.feed.load_more()?;

        // Fetch more based on that feed range
        match GLOBALS.feed.get_feed_kind() {
            FeedKind::List(_, _) => {
                // Subscribe on the minions for that missing chunk
                for relay_assignment in GLOBALS.relay_picker.relay_assignments_iter() {
                    // Ask relay to subscribe to the missing chunk
                    let _ = self.to_minions.send(ToMinionMessage {
                        target: relay_assignment.relay_url.as_str().to_owned(),
                        payload: ToMinionPayload {
                            job_id: 0,
                            detail: ToMinionPayloadDetail::TempSubscribeGeneralFeedChunk(anchor),
                        },
                    });
                }
            }
            FeedKind::Inbox(_) => {
                let relays: Vec<RelayUrl> = Relay::choose_relay_urls(Relay::READ, |_| true)?;
                // Subscribe on each of these relays
                for relay in relays.iter() {
                    // Subscribe
                    self.engage_minion(
                        relay.to_owned(),
                        vec![RelayJob {
                            reason: RelayConnectionReason::FetchInbox,
                            payload: ToMinionPayload {
                                job_id: rand::random::<u64>(),
                                detail: ToMinionPayloadDetail::TempSubscribeInboxFeedChunk(anchor),
                            },
                        }],
                    )
                    .await?;
                }
            }
            FeedKind::Person(pubkey) => {
                // Get write relays for the person
                let relays: Vec<RelayUrl> =
                    relay::get_best_relays_fixed(pubkey, RelayUsage::Outbox)?;
                // Subscribe on each of those write relays
                for relay in relays.iter() {
                    // Subscribe
                    self.engage_minion(
                        relay.to_owned(),
                        vec![RelayJob {
                            reason: RelayConnectionReason::SubscribePerson,
                            payload: ToMinionPayload {
                                job_id: rand::random::<u64>(),
                                detail: ToMinionPayloadDetail::TempSubscribePersonFeedChunk {
                                    pubkey,
                                    anchor,
                                },
                            },
                        }],
                    )
                    .await?;
                }
            }
            _ => (), // other feeds can't load more
        }

        Ok(())
    }

    /// Process approved nip46 server operation
    pub async fn nip46_server_op_approval_response(
        &mut self,
        pubkey: PublicKey,
        parsed_command: ParsedCommand,
        approval: Approval,
    ) -> Result<(), Error> {
        // Clear the request
        GLOBALS.pending.take_nip46_request(&pubkey, &parsed_command);

        // Handle the request
        if let Some(mut server) = GLOBALS.storage.read_nip46server(pubkey)? {
            match parsed_command.method.as_str() {
                "sign_event" => server.sign_approval = approval,
                "nip04_encrypt" | "nip44_encrypt" => server.encrypt_approval = approval,
                "nip04_decrypt" | "nip44_decrypt" => server.decrypt_approval = approval,
                "nip44_get_key" => {
                    server.encrypt_approval = approval;
                    server.decrypt_approval = approval;
                }
                _ => {}
            }

            // Save back
            GLOBALS.storage.write_nip46server(&server, None)?;

            // Handle it
            server.handle(&parsed_command)?;
        }

        Ok(())
    }

    /// Trigger the relay picker to find relays for people not fully covered
    pub async fn refresh_scores_and_pick_relays(&mut self) -> Result<(), Error> {
        // When manually doing this, we refresh person_relay scores first which
        // often change if the user just added follows.
        GLOBALS.relay_picker.refresh_person_relay_scores().await?;

        // Then pick
        self.pick_relays().await;

        Ok(())
    }

    pub fn finish_job(
        &mut self,
        relay_url: RelayUrl,
        job_id: Option<u64>,                   // if by job id
        reason: Option<RelayConnectionReason>, // by reason
    ) -> Result<(), Error> {
        if let Some(job_id) = job_id {
            if job_id == 0 {
                return Ok(());
            }

            if let Some(mut refmut) = GLOBALS.connected_relays.get_mut(&relay_url) {
                // Remove job by job_id
                refmut
                    .value_mut()
                    .retain(|job| job.payload.job_id != job_id);
            }
        } else if let Some(reason) = reason {
            if let Some(mut refmut) = GLOBALS.connected_relays.get_mut(&relay_url) {
                // Remove job by reason
                refmut.value_mut().retain(|job| job.reason != reason);
            }
        }

        Ok(())
    }

    /// Post a TextNote (kind 1) event
    pub async fn post(
        &mut self,
        content: String,
        tags: Vec<Tag>,
        in_reply_to: Option<Id>,
        annotation: bool,
        dm_channel: Option<DmChannel>,
    ) -> Result<(), Error> {
        let author = match GLOBALS.identity.public_key() {
            Some(pk) => pk,
            None => {
                tracing::warn!("No public key! Not posting");
                return Ok(());
            }
        };

        // Prepare events for posting
        let mut prepared_events = match dm_channel {
            Some(channel) => {
                if channel.can_use_nip17() {
                    crate::post::prepare_post_nip17(author, content, tags, channel, annotation)?
                } else {
                    crate::post::prepare_post_nip04(author, content, channel, annotation)?
                }
            }
            None => {
                crate::post::prepare_post_normal(author, content, tags, in_reply_to, annotation)?
            }
        };

        // Post them
        for (event, relay_urls) in prepared_events.drain(..) {
            // Process this event locally (ignore any error)
            let _ = crate::process::process_new_event(&event, None, None, false, false);

            // Engage minions to post
            for url in relay_urls {
                // Send it the event to post
                tracing::debug!("Asking {} to post", &url);

                self.engage_minion(
                    url.clone(),
                    vec![RelayJob {
                        reason: RelayConnectionReason::PostEvent,
                        payload: ToMinionPayload {
                            job_id: rand::random::<u64>(),
                            detail: ToMinionPayloadDetail::PostEvents(vec![event.clone()]),
                        },
                    }],
                )
                .await?;
            }
        }

        Ok(())
    }

    pub async fn post_again(&mut self, event: Event) -> Result<(), Error> {
        let relay_urls = relay::relays_for_event(&event)?;

        for url in relay_urls {
            // Send it the event to post
            tracing::debug!("Asking {} to post", &url);

            self.engage_minion(
                url.clone(),
                vec![RelayJob {
                    reason: RelayConnectionReason::PostEvent,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvents(vec![event.clone()]),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    pub async fn post_nip46_event(
        &mut self,
        event: Event,
        relays: Vec<RelayUrl>,
    ) -> Result<(), Error> {
        for url in relays {
            // Send it the event to post
            tracing::debug!("Asking {} to post nostrconnect", &url);

            self.engage_minion(
                url.clone(),
                vec![RelayJob {
                    reason: RelayConnectionReason::PostNostrConnect,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvents(vec![event.clone()]),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    /// Prune the cache (downloaded files)
    pub async fn prune_cache() -> Result<(), Error> {
        GLOBALS
            .status_queue
            .write()
            .write("Pruning cache, please be patient..".to_owned());

        let age = Duration::new(
            GLOBALS.storage.read_setting_cache_prune_period_days() * 60 * 60 * 24,
            0,
        );

        let count = GLOBALS.fetcher.prune(age).await?;

        GLOBALS
            .status_queue
            .write()
            .write(format!("Cache has been pruned. {} files removed.", count));

        Ok(())
    }

    /// Prune the database (events and more)
    pub fn prune_database() -> Result<(), Error> {
        GLOBALS
            .status_queue
            .write()
            .write("Pruning database, please be patient..".to_owned());

        let now = Unixtime::now();
        let then = now
            - Duration::new(
                GLOBALS.storage.read_setting_prune_period_days() * 60 * 60 * 24,
                0,
            );
        let count = GLOBALS.storage.prune(then)?;

        GLOBALS.status_queue.write().write(format!(
            "Database has been pruned. {} events removed.",
            count
        ));

        Ok(())
    }

    /// Publish the user's specified PersonList
    pub async fn push_person_list(&mut self, list: PersonList) -> Result<(), Error> {
        let metadata = match GLOBALS.storage.get_person_list_metadata(list)? {
            Some(m) => m,
            None => return Ok(()),
        };

        let event = GLOBALS.people.generate_person_list_event(list).await?;

        // process event locally
        crate::process::process_new_event(&event, None, None, false, false)?;

        // Push to all of the relays we post to
        let relays: Vec<Relay> = Relay::choose_relays(Relay::WRITE, |_| true)?;
        for relay in relays {
            // Send it the event to pull our followers
            tracing::debug!("Pushing PersonList={} to {}", metadata.title, &relay.url);

            self.engage_minion(
                relay.url.clone(),
                vec![RelayJob {
                    reason: RelayConnectionReason::PostContacts,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvents(vec![event.clone()]),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    /// Publish the user's metadata
    pub async fn push_metadata(&mut self, metadata: Metadata) -> Result<(), Error> {
        let public_key = match GLOBALS.identity.public_key() {
            Some(pk) => pk,
            None => return Err((ErrorKind::NoPrivateKey, file!(), line!()).into()), // not even a public key
        };

        let pre_event = PreEvent {
            pubkey: public_key,
            created_at: Unixtime::now(),
            kind: EventKind::Metadata,
            tags: vec![],
            content: serde_json::to_string(&metadata)?,
        };

        let event = GLOBALS.identity.sign_event(pre_event)?;

        // Push to all of the relays we post to
        let relays: Vec<Relay> = Relay::choose_relays(Relay::WRITE, |_| true)?;
        for relay in relays {
            // Send it the event to pull our followers
            tracing::debug!("Pushing Metadata to {}", &relay.url);

            self.engage_minion(
                relay.url.clone(),
                vec![RelayJob {
                    reason: RelayConnectionReason::PostMetadata,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvents(vec![event.clone()]),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    /// Rank a relay from 0 to 9.  The default rank is 3.  A rank of 0 means the relay will not be used.
    /// This represent a user's judgement, and is factored into how suitable a relay is for various
    /// purposes.
    pub fn rank_relay(relay_url: RelayUrl, rank: u8) -> Result<(), Error> {
        if let Some(mut relay) = GLOBALS.storage.read_relay(&relay_url, None)? {
            relay.rank = rank as u64;
            GLOBALS.storage.write_relay(&relay, None)?;
        }
        Ok(())
    }

    /// Refresh metadata for everybody who is followed
    /// This gets it whether we had it or not. Because it might have changed.
    pub async fn refresh_subscribed_metadata(&mut self) -> Result<(), Error> {
        let mut pubkeys = GLOBALS.people.get_subscribed_pubkeys();

        // add own pubkey as well
        if let Some(pubkey) = GLOBALS.identity.public_key() {
            pubkeys.push(pubkey)
        }

        let mut map: HashMap<RelayUrl, Vec<PublicKey>> = HashMap::new();

        // Sort the people into the relays we will find their metadata at
        for pubkey in &pubkeys {
            for relay in relay::get_best_relays_fixed(*pubkey, RelayUsage::Outbox)?.drain(..) {
                map.entry(relay)
                    .and_modify(|e| e.push(*pubkey))
                    .or_insert_with(|| vec![*pubkey]);
            }
        }

        for (url, pubkeys) in map.drain() {
            self.engage_minion(
                url.clone(),
                vec![RelayJob {
                    reason: RelayConnectionReason::FetchMetadata,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::TempSubscribeMetadata(pubkeys),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    /// Repost a post by `Id`
    pub async fn repost(&mut self, id: Id) -> Result<(), Error> {
        let reposted_event = match GLOBALS.storage.read_event(id)? {
            Some(event) => event,
            None => {
                GLOBALS
                    .status_queue
                    .write()
                    .write("Cannot repost - cannot find event.".to_owned());
                return Ok(());
            }
        };

        let relay_url = {
            let seen_on = GLOBALS.storage.get_event_seen_on_relay(reposted_event.id)?;
            if seen_on.is_empty() {
                // FIXME: is this the right way to pick this relay?
                relay::recommended_relay_hint(id)?.map(|rr| rr.to_unchecked_url())
            } else {
                seen_on.first().map(|(rurl, _)| rurl.to_unchecked_url())
            }
        };

        let kind: EventKind;
        let mut tags: Vec<Tag> = vec![
            Tag::new_pubkey(reposted_event.pubkey, None, None),
            Tag::new_event(id, relay_url.clone(), None),
        ];

        if reposted_event.kind != EventKind::TextNote {
            kind = EventKind::GenericRepost;

            // Add 'k' tag
            tags.push(Tag::new_kind(reposted_event.kind));

            if reposted_event.kind.is_replaceable() {
                let ea = EventAddr {
                    d: reposted_event.parameter().unwrap_or("".to_string()),
                    relays: match relay_url {
                        Some(url) => vec![url.clone()],
                        None => vec![],
                    },
                    kind: reposted_event.kind,
                    author: reposted_event.pubkey,
                };
                // Add 'a' tag
                tags.push(Tag::new_address(&ea, None));
            }
        } else {
            kind = EventKind::Repost;
        }

        let event = {
            let public_key = match GLOBALS.identity.public_key() {
                Some(pk) => pk,
                None => {
                    tracing::warn!("No public key! Not posting");
                    return Ok(());
                }
            };

            if GLOBALS.storage.read_setting_set_client_tag() {
                tags.push(Tag::new(&["client", "gossip"]));
            }

            let pre_event = PreEvent {
                pubkey: public_key,
                created_at: Unixtime::now(),
                kind,
                tags,
                content: serde_json::to_string(&reposted_event)?,
            };

            let powint = GLOBALS.storage.read_setting_pow();
            if powint > 0 {
                let (work_sender, work_receiver) = mpsc::channel();
                std::thread::spawn(move || {
                    work_logger(work_receiver, powint);
                });
                GLOBALS
                    .identity
                    .sign_event_with_pow(pre_event, powint, Some(work_sender))?
            } else {
                GLOBALS.identity.sign_event(pre_event)?
            }
        };

        // Process this event locally
        crate::process::process_new_event(&event, None, None, false, false)?;

        // Determine which relays to post this to
        let mut relay_urls: Vec<RelayUrl> = Vec::new();
        {
            // Get all of the relays that we write to
            let write_relay_urls: Vec<RelayUrl> = Relay::choose_relay_urls(Relay::WRITE, |_| true)?;
            relay_urls.extend(write_relay_urls);
            relay_urls.sort();
            relay_urls.dedup();
        }

        for url in relay_urls {
            // Send it the event to post
            tracing::debug!("Asking {} to (re)post", &url);

            self.engage_minion(
                url.clone(),
                vec![RelayJob {
                    reason: RelayConnectionReason::PostEvent,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvents(vec![event.clone()]),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    /// Search people and notes in the local database.
    /// Search results eventually arrive in `GLOBALS.people_search_results` and `GLOBALS.note_search_results`
    pub async fn search(mut text: String) -> Result<(), Error> {
        if text.len() < 2 {
            GLOBALS
                .status_queue
                .write()
                .write("You must enter at least 2 characters to search.".to_string());
            return Ok(());
        }
        text = text.to_lowercase();

        let mut people_search_results: Vec<Person> = Vec::new();
        let mut note_search_results: Vec<Event> = Vec::new();

        // If a nostr: url, strip the 'nostr:' part
        if text.len() >= 6 && &text[0..6] == "nostr:" {
            text = text.split_off(6);
        }

        if let Some(nb32) = NostrBech32::try_from_string(&text) {
            match nb32 {
                NostrBech32::EventAddr(ea) => {
                    let mut filter = Filter::new();
                    filter.add_event_kind(ea.kind);
                    filter.add_author(&ea.author.into());

                    if let Some(event) = GLOBALS
                        .storage
                        .find_events_by_filter(&filter, |event| {
                            event.tags.iter().any(|tag| {
                                if let Ok(d) = tag.parse_identifier() {
                                    if d == ea.d {
                                        return true;
                                    }
                                }
                                false
                            })
                        })?
                        .first()
                    {
                        note_search_results.push(event.clone());
                    } else {
                        let _ = GLOBALS
                            .to_overlord
                            .send(ToOverlordMessage::FetchEventAddr(ea.to_owned()));

                        // FIXME - this requires eventaddr comparison on process.rs
                        // Remember we are searching for this event, so when it comes in
                        // it can get added to GLOBALS.note_search_results
                        // GLOBALS.event_addrs_being_searched_for.write().push(ea.to_owned());
                    }
                }
                NostrBech32::EventPointer(ep) => {
                    if let Some(event) = GLOBALS.storage.read_event(ep.id)? {
                        note_search_results.push(event);
                    } else {
                        let relays: Vec<RelayUrl> = ep
                            .relays
                            .iter()
                            .filter_map(|r| RelayUrl::try_from_unchecked_url(r).ok())
                            .collect();

                        let _ = GLOBALS
                            .to_overlord
                            .send(ToOverlordMessage::FetchEvent(ep.id, relays));

                        // Remember we are searching for this event, so when it comes in
                        // it can get added to GLOBALS.note_search_results
                        GLOBALS.events_being_searched_for.write().push(ep.id);
                    }
                }
                NostrBech32::Id(id) => {
                    if let Some(event) = GLOBALS.storage.read_event(id)? {
                        note_search_results.push(event);
                    }
                    // else we can't go find it, we don't know which relays to ask.
                }
                NostrBech32::Profile(prof) => {
                    if let Some(person) = PersonTable::read_record(prof.pubkey, None)? {
                        people_search_results.push(person);
                    } else {
                        // Create person from profile
                        // fetch data on person
                    }
                }
                NostrBech32::Pubkey(pk) => {
                    if let Some(person) = PersonTable::read_record(pk, None)? {
                        people_search_results.push(person);
                    } else {
                        // Create person from pubkey
                        // fetch data on person
                    }
                }
                NostrBech32::Relay(_relay) => (),
            }
        }

        people_search_results.extend(PersonTable::filter_records(
            |p| {
                if let Some(metadata) = p.metadata() {
                    if let Ok(s) = serde_json::to_string(&metadata) {
                        if s.to_lowercase().contains(&text) {
                            return true;
                        }
                    }
                }

                if let Some(petname) = &p.petname {
                    if petname.to_lowercase().contains(&text) {
                        return true;
                    }
                }

                false
            },
            None,
        )?);

        note_search_results.extend(GLOBALS.storage.search_events(&text)?);

        *GLOBALS.people_search_results.write() = people_search_results;
        *GLOBALS.note_search_results.write() = note_search_results;

        Ok(())
    }

    /// Set a particular person as active in the `People` structure. This affects the results of
    /// some functions of that structure
    pub async fn set_active_person(pubkey: PublicKey) -> Result<(), Error> {
        GLOBALS.people.set_active_person(pubkey).await?;
        Ok(())
    }

    async fn set_dm_channel(&mut self, dmchannel: DmChannel) -> Result<(), Error> {
        // subscribe to channel on outbox and inbox relays
        //   outbox: you may have written them there. Other clients may have too.
        //   inbox: they may have put theirs here for you to pick up.
        let relays: Vec<Relay> = GLOBALS
            .storage
            .filter_relays(|r| r.has_usage_bits(Relay::OUTBOX) || r.has_usage_bits(Relay::INBOX))?;
        for relay in relays.iter() {
            // Subscribe
            self.engage_minion(
                relay.url.to_owned(),
                vec![RelayJob {
                    reason: RelayConnectionReason::FetchDirectMessages,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeDmChannel(dmchannel.clone()),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    async fn set_person_feed(&mut self, pubkey: PublicKey, anchor: Unixtime) -> Result<(), Error> {
        let relays: Vec<RelayUrl> = relay::get_best_relays_fixed(pubkey, RelayUsage::Outbox)?;

        for relay in relays.iter() {
            // Subscribe
            self.engage_minion(
                relay.to_owned(),
                vec![RelayJob {
                    reason: RelayConnectionReason::SubscribePerson,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribePersonFeed(pubkey, anchor),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    /// This function:
    ///   1. Sets GLOBALS.feed thread_parent to the highest locally connected event
    ///   2. Engages the Seeker to climb ancestors from that event
    ///   3. Subscribes to replies
    ///
    /// Note that seprately the UI constructs the thread view from local data including
    /// relationships that are built by process.rs as events flow in.
    async fn set_thread_feed(
        &mut self,
        id: Id,
        referenced_by: Id,
        author: Option<PublicKey>,
    ) -> Result<(), Error> {
        let eref = EventReference::Id {
            id,
            author,
            relays: vec![],
            marker: None,
        };

        let ancestors = crate::misc::get_event_ancestors(eref)?;

        // Set thread parent
        if let Some(ref event) = ancestors.highest_connected_local {
            // ... to the highest local event
            GLOBALS.feed.set_thread_parent(event.id);
        } else {
            // ... else to the event itself (note that it might not be local)
            GLOBALS.feed.set_thread_parent(id);
        }

        let num_relays_per_person = GLOBALS.storage.read_setting_num_relays_per_person();

        // If we don't have it all, seek the next higher ancestor
        if ancestors.highest_connected_remote.is_some() {
            // (it won't go higher right now, but if the user clicks they can climb the thread)
            // FIXME: keep climbing somehow once this comes in.

            // Let's first get additional relays the event might be on
            let mut bonus_relays: Vec<RelayUrl> = Vec::new();

            if let Some(highest_event) = ancestors.highest_connected_local {
                // Include the relays where the event was seen
                bonus_relays.extend(
                    GLOBALS
                        .storage
                        .get_event_seen_on_relay(id)?
                        .drain(..)
                        .take(num_relays_per_person as usize + 1)
                        .map(|(url, _time)| url),
                );

                // Include the OUTBOX relays of people tagged in the highest event
                for (pk, opthint, _optmarker) in highest_event.people() {
                    if let Some(url) = opthint {
                        bonus_relays.push(url);
                    } else {
                        let tagged_person_relays: Vec<RelayUrl> =
                            relay::get_best_relays_fixed(pk, RelayUsage::Outbox)?;
                        bonus_relays.extend(tagged_person_relays);
                    }
                }

                // Include relay hints in the highest event 'e' tags
                for eref in highest_event.referred_events() {
                    if let EventReference::Id {
                        id: _,
                        author: _,
                        relays: tagrelays,
                        marker: _,
                    } = eref
                    {
                        bonus_relays.extend(tagrelays);
                    }
                }
            } else {
                // We don't have the referenced event itself.

                // Include the relays where the referencing event was seen.
                bonus_relays.extend(
                    GLOBALS
                        .storage
                        .get_event_seen_on_relay(referenced_by)?
                        .drain(..)
                        .take(num_relays_per_person as usize + 1)
                        .map(|(url, _time)| url),
                );

                // Include the relays of the author of the referencing event
                if let Some(pk) = author {
                    let author_relays: Vec<RelayUrl> =
                        relay::get_best_relays_fixed(pk, RelayUsage::Outbox)?;
                    bonus_relays.extend(author_relays);
                }
            }

            // Clean up bonus_relays
            bonus_relays.sort();
            bonus_relays.dedup();

            match ancestors.highest_connected_remote {
                Some(EventReference::Addr(ea)) => {
                    let mut eaddr = ea.clone();
                    eaddr
                        .relays
                        .extend(bonus_relays.iter().map(|r| r.to_unchecked_url()));
                    eaddr.relays.sort();
                    eaddr.relays.dedup();
                    self.fetch_event_addr(eaddr).await?;
                }
                Some(EventReference::Id {
                    id,
                    author,
                    mut relays,
                    ..
                }) => {
                    if !relays.is_empty() {
                        relays.extend(bonus_relays);
                        relays.sort();
                        relays.dedup();
                        GLOBALS.seeker.seek_id_and_relays(id, relays, true);
                    } else if let Some(auth) = author {
                        GLOBALS
                            .seeker
                            .seek_id_and_author(id, auth, bonus_relays, true)?;
                    } else {
                        GLOBALS.seeker.seek_id(id, bonus_relays, true)?;
                    }
                }
                None => unreachable!(),
            }
        }

        // Cancel current subscriptions to replies and root_replies
        let _ = self.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload {
                job_id: 0,
                detail: ToMinionPayloadDetail::UnsubscribeReplies,
            },
        });

        // Subscribe to replies to root
        if let Some(ref root_eref) = ancestors.root {
            let relays = root_eref.copy_relays();
            for url in relays.iter() {
                // Subscribe root replies
                let jobs: Vec<RelayJob> = vec![RelayJob {
                    reason: RelayConnectionReason::ReadThread,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeRootReplies(root_eref.clone()),
                    },
                }];

                self.engage_minion(url.to_owned(), jobs).await?;
            }
        }

        // Search for replies
        {
            // Let's collect relays where replies might show up
            let mut bonus_relays: Vec<RelayUrl> = Vec::new();

            if let Some(event) = GLOBALS.storage.read_event(id)? {
                // Include all the INBOX relays of the author of the event
                bonus_relays.extend(relay::get_best_relays_min(
                    event.pubkey,
                    RelayUsage::Inbox,
                    0,
                )?);

                // Include all the relays where the event was seen
                bonus_relays.extend(
                    GLOBALS
                        .storage
                        .get_event_seen_on_relay(id)?
                        .drain(..)
                        .map(|(url, _time)| url),
                );
            } else {
                // We don't have the event itself yet.

                // Include the relays where the referencing event was seen.
                bonus_relays.extend(
                    GLOBALS
                        .storage
                        .get_event_seen_on_relay(referenced_by)?
                        .drain(..)
                        .take(num_relays_per_person as usize + 1)
                        .map(|(url, _time)| url),
                );

                // Include the inbox relays of the author of the referencing event
                if let Some(pk) = author {
                    let author_relays: Vec<RelayUrl> =
                        relay::get_best_relays_min(pk, RelayUsage::Inbox, 0)?;
                    bonus_relays.extend(author_relays);
                }
            }

            // Clean up bonus_relays
            bonus_relays.sort();
            bonus_relays.dedup();

            for url in bonus_relays.iter() {
                // Subscribe replies
                let jobs: Vec<RelayJob> = vec![RelayJob {
                    reason: RelayConnectionReason::ReadThread,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeReplies(id.into()),
                    },
                }];

                self.engage_minion(url.to_owned(), jobs).await?;
            }
        }

        Ok(())
    }

    /// This is done at startup and after the wizard.
    pub async fn start_long_lived_subscriptions(&mut self) -> Result<(), Error> {
        // Initialize the RelayPicker
        GLOBALS.relay_picker.init().await?;
        GLOBALS.connected_relays.clear();

        // Pick Relays and start Minions
        if !GLOBALS.storage.read_setting_offline() {
            self.pick_relays().await;
        }

        // Separately subscribe to our outbox events on our write relays
        self.subscribe_config(None).await?;

        // Separately subscribe to our inbox on our read relays
        // NOTE: we also do this on all dynamically connected relays since NIP-65 is
        //       not in widespread usage.
        self.subscribe_inbox(None).await?;

        // Separately subscribe to our giftwraps on our DM and INBOX relays
        self.subscribe_giftwraps().await?;

        // Separately subscribe to RelayList discovery for everyone we follow
        // who needs to seek a relay list again.
        let followed = GLOBALS.people.get_subscribed_pubkeys_needing_relay_lists();
        self.subscribe_discover(followed, None).await?;

        // Separately subscribe to nostr-connect channels
        let mut relays: Vec<RelayUrl> = Vec::new();
        let servers = GLOBALS.storage.read_all_nip46servers()?;
        for server in &servers {
            relays.extend(server.relays.clone());
        }
        // Also subscribe to any unconnected nostr-connect channel
        if let Some(nip46unconnected) = GLOBALS.storage.read_nip46_unconnected_server()? {
            relays.extend(nip46unconnected.relays);
        }
        relays.sort();
        relays.dedup();
        self.subscribe_nip46(relays).await?;

        Ok(())
    }

    /// Subscribe to the user's configuration events from the given relay
    pub async fn subscribe_config(&mut self, relays: Option<Vec<RelayUrl>>) -> Result<(), Error> {
        let config_relays: Vec<RelayUrl> = match relays {
            Some(r) => r,
            None => Relay::choose_relay_urls(Relay::WRITE, |_| true)?,
        };
        for relay_url in config_relays.iter() {
            self.engage_minion(
                relay_url.to_owned(),
                vec![RelayJob {
                    reason: RelayConnectionReason::Config,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeConfig,
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    /// Subscribe to the multiple user's relay lists (optionally on the given relays, otherwise using
    /// theconfigured discover relays)
    ///
    /// Caller should probably check Person.relay_list_last_sought first to make sure we don't
    /// already have an in-flight request doing this.  This can be done with:
    ///    GLOBALS.people.person_needs_relay_list()
    ///    GLOBALS.people.get_subscribed_pubkeys_needing_relay_lists()
    pub async fn subscribe_discover(
        &mut self,
        pubkeys: Vec<PublicKey>,
        relays: Option<Vec<RelayUrl>>,
    ) -> Result<(), Error> {
        if pubkeys.is_empty() {
            return Ok(());
        }

        // Mark for each person that we are seeking their relay list
        // so that we don't repeat this for a while
        let now = Unixtime::now();
        let mut txn = GLOBALS.storage.get_write_txn()?;
        for pk in pubkeys.iter() {
            PersonTable::modify(*pk, |p| p.relay_list_last_sought = now.0, Some(&mut txn))?;
        }
        txn.commit()?;

        // Discover their relays
        let discover_relay_urls: Vec<RelayUrl> = match relays {
            Some(r) => r,
            None => Relay::choose_relay_urls(Relay::DISCOVER, |_| true)?,
        };
        for relay_url in discover_relay_urls.iter() {
            self.engage_minion(
                relay_url.to_owned(),
                vec![RelayJob {
                    reason: RelayConnectionReason::Discovery,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeDiscover(pubkeys.clone()),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    /// Subscribe to the user's configuration events from the given relay
    pub async fn subscribe_inbox(&mut self, relays: Option<Vec<RelayUrl>>) -> Result<(), Error> {
        let now = Unixtime::now();
        let mention_relays: Vec<RelayUrl> = match relays {
            Some(r) => r,
            None => Relay::choose_relay_urls(Relay::READ, |_| true)?,
        };
        for relay_url in mention_relays.iter() {
            self.engage_minion(
                relay_url.to_owned(),
                vec![RelayJob {
                    reason: RelayConnectionReason::FetchInbox,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeInbox(now),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    /// Subscribe to the user's giftwrap events on their DM and INBOX relays
    pub async fn subscribe_giftwraps(&mut self) -> Result<(), Error> {
        let relays: Vec<Relay> = GLOBALS
            .storage
            .filter_relays(|r| r.has_usage_bits(Relay::DM) || r.has_usage_bits(Relay::INBOX))?;

        // 30 days worth (FIXME make this a setting?)
        let after = Unixtime::now() - Duration::new(3600 * 24 * 30, 0);

        for relay in relays.iter() {
            self.engage_minion(
                relay.url.clone(),
                vec![RelayJob {
                    reason: RelayConnectionReason::Giftwraps,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeGiftwraps(after),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    /// Subscribe to nip46 nostr connect relays
    pub async fn subscribe_nip46(&mut self, relays: Vec<RelayUrl>) -> Result<(), Error> {
        for relay_url in relays.iter() {
            self.engage_minion(
                relay_url.to_owned(),
                vec![RelayJob {
                    reason: RelayConnectionReason::NostrConnect,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeNip46,
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    /// Unlock the private key with the given passphrase so that gossip can use it.
    /// This is akin to logging in.
    pub fn unlock_key(mut password: String) -> Result<(), Error> {
        if let Err(e) = GLOBALS.identity.unlock(&password) {
            tracing::error!("{}", e);
            GLOBALS
                .status_queue
                .write()
                .write("The passphrase is wrong, try again".to_owned());
        };
        password.zeroize();

        // Update public key from private key
        let public_key = GLOBALS.identity.public_key().unwrap();
        GLOBALS
            .storage
            .write_setting_public_key(&Some(public_key), None)?;

        Ok(())
    }

    /// Subscribe, fetch, and update metadata for the person
    pub async fn update_metadata(&mut self, pubkey: PublicKey) -> Result<(), Error> {
        // Indicate that we are doing this, as the People manager wants to know
        // for it's retry logic
        GLOBALS.people.metadata_fetch_initiated(&[pubkey]);

        let best_relays = relay::get_best_relays_fixed(pubkey, RelayUsage::Outbox)?;

        // we do 1 more than num_relays_per_person, which is really for main posts,
        // since metadata is more important and I didn't want to bother with
        // another setting.
        for relay_url in best_relays.iter() {
            self.engage_minion(
                relay_url.to_owned(),
                vec![RelayJob {
                    reason: RelayConnectionReason::FetchMetadata,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::TempSubscribeMetadata(vec![pubkey]),
                    },
                }],
            )
            .await?;
        }

        // Mark in globals that we want to recheck their nip-05 when that metadata
        // comes in
        GLOBALS.people.recheck_nip05_on_update_metadata(&pubkey);

        Ok(())
    }

    /// Subscribe, fetch, and update metadata for the people
    pub async fn update_metadata_in_bulk(
        &mut self,
        mut pubkeys: Vec<PublicKey>,
    ) -> Result<(), Error> {
        // Indicate that we are doing this, as the People manager wants to know
        // for it's retry logic
        GLOBALS.people.metadata_fetch_initiated(&pubkeys);

        let mut map: HashMap<RelayUrl, Vec<PublicKey>> = HashMap::new();
        for pubkey in pubkeys.drain(..) {
            let best_relays = relay::get_best_relays_fixed(pubkey, RelayUsage::Outbox)?;
            for relay_url in best_relays.iter() {
                map.entry(relay_url.to_owned())
                    .and_modify(|entry| entry.push(pubkey))
                    .or_insert_with(|| vec![pubkey]);
            }
        }
        for (relay_url, pubkeys) in map.drain() {
            self.engage_minion(
                relay_url.clone(),
                vec![RelayJob {
                    reason: RelayConnectionReason::FetchMetadata,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::TempSubscribeMetadata(pubkeys),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    /// Update the local person list from the last event received.
    pub async fn update_person_list(&mut self, list: PersonList, merge: bool) -> Result<(), Error> {
        // we cannot do anything without an identity setup first
        let my_pubkey = match GLOBALS.storage.read_setting_public_key() {
            Some(pk) => pk,
            None => {
                GLOBALS
                    .status_queue
                    .write()
                    .write("You cannot update person lists without an identity".to_string());
                return Ok(());
            }
        };

        // Get the metadata first
        let mut metadata = match GLOBALS.storage.get_person_list_metadata(list)? {
            Some(m) => m,
            None => return Ok(()),
        };

        // Load the latest PersonList event from the database
        let event = {
            if let Some(event) = GLOBALS.storage.get_replaceable_event(
                list.event_kind(),
                my_pubkey,
                &metadata.dtag,
            )? {
                event.clone()
            } else {
                GLOBALS
                    .status_queue
                    .write()
                    .write("Could not find a person-list event to update from".to_string());
                return Ok(()); // we have no event to update from, so we are done
            }
        };

        let now = Unixtime::now();

        let mut txn = GLOBALS.storage.get_write_txn()?;

        let mut entries: Vec<(PublicKey, Private)> = Vec::new();

        // Public entries
        for tag in &event.tags {
            if let Ok((pubkey, rurl, petname)) = tag.parse_pubkey() {
                // If our list is marked private, move these public entries to private ones
                let private = metadata.private;

                // Save the pubkey
                entries.push((pubkey.to_owned(), private));

                // Deal with recommended_relay_urls and petnames
                if list == PersonList::Followed {
                    Self::integrate_rru_and_petname(
                        &pubkey, &rurl, &petname, now, merge, &mut txn,
                    )?;
                }
            }

            if let Ok(title) = tag.parse_title() {
                metadata.title = title.to_owned();
            }
        }

        if list != PersonList::Followed && !event.content.is_empty() {
            if GLOBALS.identity.is_unlocked() {
                // Private entries
                let decrypted_content = GLOBALS.identity.decrypt(&my_pubkey, &event.content)?;

                let tags: Vec<Tag> = serde_json::from_str(&decrypted_content)?;

                for tag in &tags {
                    if let Ok((pubkey, _, _)) = tag.parse_pubkey() {
                        // Save the pubkey
                        entries.push((pubkey.to_owned(), Private(true)));
                    }
                    if let Ok(title) = tag.parse_title() {
                        metadata.title = title.to_owned();
                    }
                }
            } else {
                // If we need to decrypt contents but can't, let them know we couldn't read that part
                GLOBALS.status_queue.write().write(
                    format!("Since you are not logged in, the encrypted contents of the list {} will not be processed.", metadata.title),
                );
            }
        }

        if !merge {
            GLOBALS.storage.clear_person_list(list, Some(&mut txn))?;
        }

        for (pubkey, private) in &entries {
            GLOBALS
                .storage
                .add_person_to_list(pubkey, list, *private, Some(&mut txn))?;
            GLOBALS.ui_people_to_invalidate.write().push(*pubkey);
        }

        let last_edit = if merge { now } else { event.created_at };

        metadata.last_edit_time = last_edit;
        metadata.len = if merge {
            GLOBALS.storage.get_people_in_list(list)?.len()
        } else {
            entries.len()
        };

        GLOBALS
            .storage
            .set_person_list_metadata(list, &metadata, Some(&mut txn))?;

        txn.commit()?;

        // Pick relays again
        if list.subscribe() {
            // Refresh person-relay scores
            GLOBALS.relay_picker.refresh_person_relay_scores().await?;

            // Then pick
            self.pick_relays().await;
        }

        Ok(())
    }

    fn integrate_rru_and_petname(
        pubkey: &PublicKey,
        recommended_relay_url: &Option<UncheckedUrl>,
        petname: &Option<String>,
        now: Unixtime,
        merge: bool,
        txn: &mut RwTxn,
    ) -> Result<(), Error> {
        // If there is a URL
        if let Some(url) = recommended_relay_url
            .as_ref()
            .and_then(|rru| RelayUrl::try_from_unchecked_url(rru).ok())
        {
            // Save relay if missing
            GLOBALS.storage.write_relay_if_missing(&url, Some(txn))?;

            // Modify person_relay
            GLOBALS.storage.modify_person_relay(
                *pubkey,
                &url,
                |pr| pr.last_suggested = Some(now.0 as u64),
                Some(txn),
            )?;
        }

        // Handle petname
        if merge && petname.is_none() {
            // In this case, we leave any existing petname, so no need to load the
            // person record. But we need to ensure the person exists
            PersonTable::create_record_if_missing(*pubkey, Some(txn))?;
        } else {
            PersonTable::modify(
                *pubkey,
                |person| {
                    if *petname != person.petname {
                        if petname.is_some() && petname != &Some("".to_string()) {
                            person.petname = petname.clone();
                        } else if !merge {
                            // In overwrite mode, clear to None
                            person.petname = None;
                        }
                    }
                },
                Some(txn),
            )?;
        }

        Ok(())
    }

    /// Update the relay. This saves the new relay and also adjusts active
    /// subscriptions based on the changes.
    pub async fn update_relay(&mut self, old: Relay, new: Relay) -> Result<(), Error> {
        if old.url != new.url {
            return Err(ErrorKind::CannotUpdateRelayUrl.into());
        }

        // Write new
        GLOBALS.storage.write_relay(&new, None)?;

        // No minion action if we are offline
        if GLOBALS.storage.read_setting_offline() {
            return Ok(());
        }

        // If rank went to zero
        if old.rank != 0 && new.rank == 0 {
            // Close minion for this relay
            self.drop_relay(new.url.clone())?;
            return Ok(());
        }

        // Remember if we need to subscribe (+1) or unsubscribe (-1)
        let mut inbox: i8 = 0;
        let mut config: i8 = 0;
        let mut discover: i8 = 0;

        // if usage bits changed
        if old.get_usage_bits() != new.get_usage_bits() {
            if old.has_usage_bits(Relay::READ) && !new.has_usage_bits(Relay::READ) {
                inbox = -1;
            } else if !old.has_usage_bits(Relay::READ) && new.has_usage_bits(Relay::READ) {
                inbox = 1;
            }

            if old.has_usage_bits(Relay::WRITE) && !new.has_usage_bits(Relay::WRITE) {
                config = -1;
            } else if !old.has_usage_bits(Relay::WRITE) && new.has_usage_bits(Relay::WRITE) {
                config = 1;
            }

            if old.has_usage_bits(Relay::DISCOVER) && !new.has_usage_bits(Relay::DISCOVER) {
                discover = -1;
            } else if !old.has_usage_bits(Relay::DISCOVER) && new.has_usage_bits(Relay::DISCOVER) {
                discover = 1;
            }
        }

        // If rank came from zero, start subs on this relay
        if old.rank == 0 && new.rank != 0 {
            // Start minion for this relay
            if new.has_usage_bits(Relay::READ) {
                inbox = 1;
            }
            if new.has_usage_bits(Relay::WRITE) {
                config = 1;
            }
            if new.has_usage_bits(Relay::DISCOVER) {
                discover = 1;
            }
        }

        match inbox {
            -1 => (), // TBD unsubscribe_inbox
            1 => {
                if let Some(pubkey) = GLOBALS.identity.public_key() {
                    // Modify self person_relay
                    GLOBALS.storage.modify_person_relay(
                        pubkey,
                        &new.url,
                        |pr| pr.read = true,
                        None,
                    )?;

                    // Subscribe to inbox on this inbox relay
                    self.subscribe_inbox(Some(vec![new.url.clone()])).await?;
                }
            }
            _ => (),
        }

        match config {
            -1 => (), // TBD unsubscribe_config
            1 => {
                if let Some(pubkey) = GLOBALS.identity.public_key() {
                    // Modify self person_relay
                    GLOBALS.storage.modify_person_relay(
                        pubkey,
                        &new.url,
                        |pr| pr.write = true,
                        None,
                    )?;

                    // Subscribe to config on this outbox relay
                    self.subscribe_config(Some(vec![new.url.clone()])).await?;
                }
            }
            _ => (),
        }

        match discover {
            -1 => (), // Discover subscriptions are temp / short-lived, so no action needed.
            1 => {
                let pubkeys = GLOBALS.people.get_subscribed_pubkeys_needing_relay_lists();
                self.subscribe_discover(pubkeys, Some(vec![new.url.clone()]))
                    .await?;
            }
            _ => (),
        }

        Ok(())
    }

    /// Set which notes are currently visible to the user. This is used to modify subscriptions
    /// that query for likes, zaps, and deletions. Such subscriptions only query for that data
    /// for events currently in view, to keep them small.
    ///
    /// WARNING: DO NOT CALL TOO OFTEN or relays will hate you.
    pub async fn visible_notes_changed(&mut self, mut visible: Vec<Id>) -> Result<(), Error> {
        // Work out which relays to use to find augments for which ids
        let mut augment_subs: HashMap<RelayUrl, Vec<Id>> = HashMap::new();
        for id in visible.drain(..) {
            if let Some(event) = GLOBALS.storage.read_event(id)? {
                let relays = relay::relays_for_seeking_replies(&event)?;
                for relay_url in relays {
                    augment_subs
                        .entry(relay_url)
                        .and_modify(|vec| {
                            if !vec.contains(&id) {
                                vec.push(id)
                            }
                        })
                        .or_insert(vec![id]);
                }
            }
        }

        // Create jobs for minions
        for (relay_url, ids) in augment_subs.drain() {
            let ids_hex: Vec<IdHex> = ids.iter().map(|i| (*i).into()).collect();

            self.engage_minion(
                relay_url,
                vec![RelayJob {
                    reason: RelayConnectionReason::FetchAugments,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeAugments(ids_hex),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    /// Start a Zap on the note with Id and author PubKey, at the given lnurl.
    /// This eventually sets `GLOBALS.current_zap`, after which you can complete it
    /// with Zap()
    pub async fn zap_start(
        &mut self,
        id: Id,
        target_pubkey: PublicKey,
        lnurl: UncheckedUrl,
    ) -> Result<(), Error> {
        if GLOBALS.identity.public_key().is_none() {
            tracing::warn!("You need to setup your private-key to zap.");
            GLOBALS
                .status_queue
                .write()
                .write("You need to setup your private-key to zap.".to_string());
            *GLOBALS.current_zap.write() = ZapState::None;
            return Ok(());
        }

        *GLOBALS.current_zap.write() = ZapState::CheckingLnurl(id, target_pubkey, lnurl.clone());

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::new(15, 0))
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .build()?;

        // Convert the lnurl UncheckedUrl to a Url
        let url = nostr_types::Url::try_from_unchecked_url(&lnurl)?;

        // Read the PayRequestData from the lnurl
        let response = client.get(url.as_str()).send().await?;
        let text = response.text().await?;
        let prd: PayRequestData = match serde_json::from_str(&text) {
            Ok(prd) => prd,
            Err(e) => {
                tracing::error!("Zap pay request data invalid: {}, {}", text, e);
                GLOBALS
                    .status_queue
                    .write()
                    .write(format!("Zap pay request data invalid: {}, {}", text, e));
                *GLOBALS.current_zap.write() = ZapState::None;
                return Ok(());
            }
        };

        // Verify it supports nostr
        if prd.allows_nostr != Some(true) {
            tracing::warn!("Zap wallet does not support nostr, trying anyways...");
            GLOBALS
                .status_queue
                .write()
                .write("Zap wallet does not support nostr, trying anyways...".to_string());
        }

        *GLOBALS.current_zap.write() = ZapState::SeekingAmount(id, target_pubkey, prd, lnurl);

        Ok(())
    }

    /// Complete a zap on the note with Id and author PublicKey by setting a value and a comment.
    pub async fn zap(
        &mut self,
        id: Id,
        target_pubkey: PublicKey,
        msats: MilliSatoshi,
        comment: String,
    ) -> Result<(), Error> {
        use serde_json::Value;

        let user_pubkey = match GLOBALS.identity.public_key() {
            Some(pk) => pk,
            None => {
                tracing::warn!("You need to setup your private-key to zap.");
                GLOBALS
                    .status_queue
                    .write()
                    .write("You need to setup your private-key to zap.".to_string());
                *GLOBALS.current_zap.write() = ZapState::None;
                return Ok(());
            }
        };

        // Make sure we are in the right zap state, and destructure it
        let (state_id, state_pubkey, prd, lnurl) = match *GLOBALS.current_zap.read() {
            ZapState::SeekingAmount(state_id, state_pubkey, ref prd, ref lnurl) => {
                (state_id, state_pubkey, prd.clone(), lnurl.clone())
            }
            _ => {
                tracing::warn!("Wrong zap state. Resetting zap state.");
                *GLOBALS.current_zap.write() = ZapState::None;
                return Ok(());
            }
        };

        // Make sure the zap we are doing matches the zap we setup previously
        if id != state_id || target_pubkey != state_pubkey {
            tracing::warn!("Zap mismatch. Resetting zap state.");
            *GLOBALS.current_zap.write() = ZapState::None;
            return Ok(());
        }

        // Validate amount bounds
        if let Some(Value::Number(n)) = prd.other.get("minSendable") {
            if let Some(u) = n.as_u64() {
                if msats.0 < u {
                    tracing::warn!("Zap amount too low. Min is {}", u);
                    GLOBALS
                        .status_queue
                        .write()
                        .write("Zap amount is too low.".to_string());
                    // leave zap state as is.
                    return Ok(());
                }
            }
        }
        if let Some(Value::Number(n)) = prd.other.get("maxSendable") {
            if let Some(u) = n.as_u64() {
                if msats.0 > u {
                    tracing::warn!("Zap amount too high. Max is {}", u);
                    GLOBALS
                        .status_queue
                        .write()
                        .write("Zap amount is too high.".to_string());
                    // leave zap state as is.
                    return Ok(());
                }
            }
        }

        // Bump the state
        *GLOBALS.current_zap.write() = ZapState::LoadingInvoice(id, target_pubkey);

        let msats_string: String = format!("{}", msats.0);

        // Convert the callback UncheckedUrl to a Url
        let callback = nostr_types::Url::try_from_unchecked_url(&prd.callback)?;

        // Get the relays to have the receipt posted to
        let relays = {
            // Start with the relays the event was seen on
            let mut relays: Vec<RelayUrl> = GLOBALS
                .storage
                .get_event_seen_on_relay(id)?
                .drain(..)
                .map(|(url, _)| url)
                .collect();

            // Add the read relays of the target person
            let target_read_relays: Vec<RelayUrl> =
                relay::get_best_relays_min(target_pubkey, RelayUsage::Inbox, 0)?;
            relays.extend(target_read_relays);

            // Add all my write relays
            let write_relay_urls: Vec<RelayUrl> = Relay::choose_relay_urls(Relay::WRITE, |_| true)?;
            relays.extend(write_relay_urls);

            if relays.is_empty() {
                *GLOBALS.current_zap.write() = ZapState::None;
                return Err(ErrorKind::NoRelay.into());
            }

            // Deduplicate
            relays.sort();
            relays.dedup();

            // Turn relays into strings for the event tag
            let relays: Vec<String> = relays.iter().map(|r| r.as_str().to_owned()).collect();
            relays
        };

        let mut relays_tag = Tag::new(&["relays"]);
        relays_tag.push_values(relays);

        // Generate the zap request event
        let pre_event = PreEvent {
            pubkey: user_pubkey,
            created_at: Unixtime::now(),
            kind: EventKind::ZapRequest,
            tags: vec![
                Tag::new_event(id, None, None),
                Tag::new_pubkey(target_pubkey, None, None),
                relays_tag,
                Tag::new(&["amount", &msats_string]),
                Tag::new(&["lnurl", lnurl.as_str()]),
            ],
            content: comment,
        };

        let event = GLOBALS.identity.sign_event(pre_event)?;
        let serialized_event = serde_json::to_string(&event)?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::new(15, 0))
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .build()?;

        let mut url = match url::Url::parse(callback.as_str()) {
            Ok(url) => url,
            Err(e) => {
                tracing::error!("{}", e);
                *GLOBALS.current_zap.write() = ZapState::None;
                return Ok(());
            }
        };

        url.query_pairs_mut()
            .clear()
            .append_pair("nostr", &serialized_event)
            .append_pair("amount", &msats_string);

        let response = client.get(url).send().await?;
        let text = response.text().await?;

        let value: serde_json::Value = serde_json::from_str(&text)?;
        if let Value::Object(map) = value {
            if let Some(Value::String(s)) = map.get("pr") {
                tracing::debug!("Zap Invoice = {}", s);
                *GLOBALS.current_zap.write() = ZapState::ReadyToPay(id, s.to_owned());
                return Ok(());
            }
        }

        *GLOBALS.current_zap.write() = ZapState::None;
        tracing::warn!("Zap invoice data not recognized: {}", text);
        GLOBALS
            .status_queue
            .write()
            .write("Zap invoice data not recognized.".to_string());

        Ok(())
    }
}

fn work_logger(work_receiver: mpsc::Receiver<u8>, powint: u8) {
    while let Ok(work) = work_receiver.recv() {
        if work >= powint {
            // Even if work > powint, it doesn't count since we declared our target.
            GLOBALS
                .status_queue
                .write()
                .write(format!("Message sent with {powint} bits of work computed."));
            break;
        } else {
            GLOBALS
                .status_queue
                .write()
                .write(format!("PoW: {work}/{powint}"));
        }
    }
}
