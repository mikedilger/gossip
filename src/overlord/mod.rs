mod minion;

use crate::comms::{
    RelayConnectionReason, RelayJob, ToMinionMessage, ToMinionPayload, ToMinionPayloadDetail,
    ToOverlordMessage,
};
use crate::dm_channel::DmChannel;
use crate::error::{Error, ErrorKind};
use crate::globals::{ZapState, GLOBALS};
use crate::people::Person;
use crate::person_relay::PersonRelay;
use crate::relay::Relay;
use crate::tags::{
    add_addr_to_tags, add_event_to_tags, add_pubkey_hex_to_tags, add_pubkey_to_tags,
    add_subject_to_tags_if_missing,
};
use gossip_relay_picker::{Direction, RelayAssignment};
use http::StatusCode;
use minion::Minion;
use nostr_types::{
    EncryptedPrivateKey, Event, EventKind, Id, IdHex, Metadata, MilliSatoshi, NostrBech32,
    PayRequestData, PreEvent, PrivateKey, Profile, PublicKey, RelayUrl, Tag, UncheckedUrl,
    Unixtime,
};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::{select, task};
use zeroize::Zeroize;

type MinionResult = Result<(), Error>;

pub struct Overlord {
    to_minions: Sender<ToMinionMessage>,
    inbox: UnboundedReceiver<ToOverlordMessage>,

    // All the minion tasks running.
    minions: task::JoinSet<Result<(), Error>>,

    // Map from minion task::Id to Url
    minions_task_url: HashMap<task::Id, RelayUrl>,
}

impl Overlord {
    pub fn new(inbox: UnboundedReceiver<ToOverlordMessage>) -> Overlord {
        let to_minions = GLOBALS.to_minions.clone();
        Overlord {
            to_minions,
            inbox,
            minions: task::JoinSet::new(),
            minions_task_url: HashMap::new(),
        }
    }

    pub async fn run(&mut self) {
        if let Err(e) = self.run_inner().await {
            tracing::error!("{}", e);
        }

        tracing::debug!("Overlord signalling UI to shutdown");

        if let Err(e) = GLOBALS.storage.sync() {
            tracing::error!("{}", e);
        } else {
            tracing::info!("LMDB synced.");
        }

        GLOBALS.shutting_down.store(true, Ordering::Relaxed);

        tracing::debug!("Overlord signalling minions to shutdown");

        // Send shutdown message to all minions (and ui)
        // If this fails, it's probably because there are no more listeners
        // so just ignore it and keep shutting down.
        let _ = self.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload {
                job_id: 0,
                detail: ToMinionPayloadDetail::Shutdown,
            },
        });

        tracing::info!("Overlord waiting for minions to all shutdown");

        // Listen on self.minions until it is empty
        while !self.minions.is_empty() {
            select! {
                _ = tokio::time::sleep(Duration::from_secs(10)) => {
                    tracing::info!("Overlord signalling minions to shutdown (again)");
                    // Send the shutdown message again
                    let _ = self.to_minions.send(ToMinionMessage {
                        target: "all".to_string(),
                        payload: ToMinionPayload {
                            job_id: 0,
                            detail: ToMinionPayloadDetail::Shutdown,
                        },
                    });
                },
                task_nextjoined = self.minions.join_next_with_id() => {
                    self.handle_task_nextjoined(task_nextjoined).await;
                }
            }
        }

        tracing::info!("Overlord confirms all minions have shutdown");
    }

    pub async fn run_inner(&mut self) -> Result<(), Error> {
        // Start the fetcher
        crate::fetcher::Fetcher::start()?;

        // Load signer from settings
        GLOBALS.signer.load_from_settings()?;

        // FIXME - if this needs doing, it should be done dynamically as
        //         new people are encountered, not batch-style on startup.
        // Create a person record for every person seen

        // Load delegation tag
        GLOBALS.delegation.load()?;

        // Initialize the relay picker
        GLOBALS.relay_picker.init().await?;

        // Pick Relays and start Minions
        if !GLOBALS.storage.read_setting_offline() {
            self.pick_relays().await;
        }

        // Separately subscribe to RelayList discovery for everyone we follow
        let discover_relay_urls: Vec<RelayUrl> = GLOBALS
            .storage
            .filter_relays(|r| r.has_usage_bits(Relay::DISCOVER))?
            .iter()
            .map(|relay| relay.url.clone())
            .collect();
        let followed = GLOBALS.people.get_followed_pubkeys();
        for relay_url in discover_relay_urls.iter() {
            self.engage_minion(
                relay_url.to_owned(),
                vec![RelayJob {
                    reason: RelayConnectionReason::Discovery,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeDiscover(followed.clone()),
                    },
                }],
            )
            .await?;
        }

        // Separately subscribe to our outbox events on our write relays
        let write_relay_urls: Vec<RelayUrl> = GLOBALS
            .storage
            .filter_relays(|r| r.has_usage_bits(Relay::WRITE))?
            .iter()
            .map(|relay| relay.url.clone())
            .collect();
        for relay_url in write_relay_urls.iter() {
            self.engage_minion(
                relay_url.to_owned(),
                vec![RelayJob {
                    reason: RelayConnectionReason::Config,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeOutbox,
                    },
                }],
            )
            .await?;
        }

        // Separately subscribe to our mentions on our read relays
        // NOTE: we also do this on all dynamically connected relays since NIP-65 is
        //       not in widespread usage.
        let read_relay_urls: Vec<RelayUrl> = GLOBALS
            .storage
            .filter_relays(|r| r.has_usage_bits(Relay::READ))?
            .iter()
            .map(|relay| relay.url.clone())
            .collect();
        for relay_url in read_relay_urls.iter() {
            self.engage_minion(
                relay_url.to_owned(),
                vec![RelayJob {
                    reason: RelayConnectionReason::FetchMentions,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeMentions,
                    },
                }],
            )
            .await?;
        }

        'mainloop: loop {
            match self.loop_handler().await {
                Ok(keepgoing) => {
                    if !keepgoing {
                        break 'mainloop;
                    }
                }
                Err(e) => {
                    // Log them and keep looping
                    tracing::error!("{}", e);
                }
            }
        }

        Ok(())
    }

    async fn pick_relays(&mut self) {
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
        // Subscribe to the general feed
        self.engage_minion(
            assignment.relay_url.clone(),
            vec![
                RelayJob {
                    reason: RelayConnectionReason::Follow,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeGeneralFeed(
                            assignment.pubkeys.clone(),
                        ),
                    },
                },
                RelayJob {
                    // Until NIP-65 is in widespread use, we should listen for mentions
                    // of us on all these relays too
                    reason: RelayConnectionReason::FetchMentions,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeMentions,
                    },
                },
            ],
        )
        .await?;

        Ok(())
    }

    async fn engage_minion(&mut self, url: RelayUrl, mut jobs: Vec<RelayJob>) -> Result<(), Error> {
        // Do not connect if we are offline
        if GLOBALS.storage.read_setting_offline() {
            return Ok(());
        }

        if jobs.is_empty() {
            return Ok(());
        }

        if let Some(mut refmut) = GLOBALS.connected_relays.get_mut(&url) {
            // We are already connected. Send it the jobs
            for job in jobs.drain(..) {
                let _ = self.to_minions.send(ToMinionMessage {
                    target: url.0.clone(),
                    payload: job.payload.clone(),
                });

                // And record
                refmut.value_mut().push(job);
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

            // And record it
            GLOBALS.connected_relays.insert(url, jobs);
        }

        Ok(())
    }

    #[allow(unused_assignments)]
    async fn loop_handler(&mut self) -> Result<bool, Error> {
        let mut keepgoing: bool = true;

        tracing::trace!("overlord looping");

        if self.minions.is_empty() {
            // Just listen on inbox
            let message = self.inbox.recv().await;
            let message = match message {
                Some(bm) => bm,
                None => {
                    // All senders dropped, or one of them closed.
                    return Ok(false);
                }
            };
            keepgoing = self.handle_message(message).await?;
        } else {
            // Listen on inbox, and dying minions
            select! {
                message = self.inbox.recv() => {
                    let message = match message {
                        Some(bm) => bm,
                        None => {
                            // All senders dropped, or one of them closed.
                            return Ok(false);
                        }
                    };
                    keepgoing = self.handle_message(message).await?;
                },
                task_nextjoined = self.minions.join_next_with_id() => {
                    self.handle_task_nextjoined(task_nextjoined).await;
                }
            }
        }

        Ok(keepgoing)
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

        // Set to not connected
        let relayjobs = GLOBALS.connected_relays.remove(&url).map(|(_, v)| v);

        let mut exclusion: u64 = 0;

        match join_result {
            Err(join_error) => {
                tracing::error!("Minion {} completed with join error: {}", &url, join_error);
                Self::bump_failure_count(&url);
                exclusion = 60;
            }
            Ok((_id, result)) => match result {
                Ok(_) => {
                    tracing::debug!("Minion {} completed", &url);
                    // no exclusion
                }
                Err(e) => {
                    Self::bump_failure_count(&url);
                    tracing::error!("Minion {} completed with error: {}", &url, e);
                    exclusion = 60;
                    if let ErrorKind::RelayRejectedUs = e.kind {
                        exclusion = 60 * 60 * 24 * 365; // don't connect again, practically
                    } else if let ErrorKind::Websocket(wserror) = e.kind {
                        if let tungstenite::error::Error::Http(response) = wserror {
                            exclusion = match response.status() {
                                StatusCode::MOVED_PERMANENTLY => 60 * 60 * 24,
                                StatusCode::PERMANENT_REDIRECT => 60 * 60 * 24,
                                StatusCode::UNAUTHORIZED => 60 * 60 * 24,
                                StatusCode::PAYMENT_REQUIRED => 60 * 60 * 24,
                                StatusCode::FORBIDDEN => 60 * 60 * 24,
                                StatusCode::NOT_FOUND => 60 * 60 * 24,
                                StatusCode::PROXY_AUTHENTICATION_REQUIRED => 60 * 60 * 24,
                                StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS => 60 * 60 * 24,
                                StatusCode::NOT_IMPLEMENTED => 60 * 60 * 24,
                                StatusCode::BAD_GATEWAY => 60 * 60 * 24,
                                s if s.as_u16() >= 400 => 90,
                                _ => 60,
                            };
                        } else if let tungstenite::error::Error::ConnectionClosed = wserror {
                            tracing::debug!("Minion {} completed", &url);
                            exclusion = 0; // was not actually an error
                        } else if let tungstenite::error::Error::Protocol(protocol_error) = wserror
                        {
                            exclusion = match protocol_error {
                                tungstenite::error::ProtocolError::ResetWithoutClosingHandshake => {
                                    30
                                }
                                _ => 120,
                            }
                        }
                    }
                }
            },
        };

        // Let the relay picker know it disconnected
        GLOBALS
            .relay_picker
            .relay_disconnected(&url, exclusion as i64);

        // We might need to act upon this minion exiting
        if !GLOBALS.shutting_down.load(Ordering::Relaxed) {
            self.recover_from_minion_exit(url, relayjobs, exclusion)
                .await;
        }
    }

    async fn recover_from_minion_exit(
        &mut self,
        url: RelayUrl,
        jobs: Option<Vec<RelayJob>>,
        exclusion: u64,
    ) {
        // For people we are following, pick relays
        if let Err(e) = GLOBALS.relay_picker.refresh_person_relay_scores().await {
            tracing::error!("Error: {}", e);
        }
        self.pick_relays().await;

        if let Some(mut jobs) = jobs {
            // If we have any persistent jobs, restart after a delaythe relay
            let persistent_jobs: Vec<RelayJob> = jobs
                .drain(..)
                .filter(|job| job.reason.persistent())
                .collect();

            if !persistent_jobs.is_empty() {
                // Do it after a delay
                std::mem::drop(tokio::spawn(async move {
                    // Delay for exclusion first
                    tracing::info!(
                        "Minion {} will restart in {} seconds to continue persistent jobs",
                        &url,
                        exclusion
                    );
                    tokio::time::sleep(Duration::new(exclusion, 0)).await;
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::ReengageMinion(url, persistent_jobs));
                }));
            }
        }
    }

    fn bump_failure_count(url: &RelayUrl) {
        if let Ok(Some(mut relay)) = GLOBALS.storage.read_relay(url) {
            relay.failure_count += 1;
            let _ = GLOBALS.storage.write_relay(&relay, None);
        }
    }

    async fn handle_message(&mut self, message: ToOverlordMessage) -> Result<bool, Error> {
        match message {
            ToOverlordMessage::AddRelay(relay_str) => {
                let relay = Relay::new(relay_str);
                GLOBALS.storage.write_relay(&relay, None)?;
            }
            ToOverlordMessage::ClearAllUsageOnRelay(relay_url) => {
                if let Some(mut relay) = GLOBALS.storage.read_relay(&relay_url)? {
                    // TODO: replace with dedicated method to clear all bits
                    relay.clear_usage_bits(
                        Relay::ADVERTISE
                            | Relay::DISCOVER
                            | Relay::INBOX
                            | Relay::OUTBOX
                            | Relay::READ
                            | Relay::WRITE,
                    );
                    GLOBALS.storage.write_relay(&relay, None)?;
                } else {
                    tracing::error!("CODE OVERSIGHT - Attempt to clear relay usage bit for a relay not in memory. It will not be saved.");
                }
            }
            ToOverlordMessage::AdjustRelayUsageBit(relay_url, bit, value) => {
                if let Some(mut relay) = GLOBALS.storage.read_relay(&relay_url)? {
                    relay.adjust_usage_bit(bit, value);
                    GLOBALS.storage.write_relay(&relay, None)?;
                } else {
                    tracing::error!("CODE OVERSIGHT - We are adjusting a relay usage bit for a relay not in memory, how did that happen? It will not be saved.");
                }
            }
            ToOverlordMessage::AdvertiseRelayList => {
                self.advertise_relay_list().await?;
            }
            ToOverlordMessage::ChangePassphrase(mut old, mut new) => {
                GLOBALS.signer.change_passphrase(&old, &new)?;
                old.zeroize();
                new.zeroize();
            }
            ToOverlordMessage::ClearFollowing => {
                self.clear_following().await?;
            }
            ToOverlordMessage::DelegationReset => {
                Self::delegation_reset().await?;
            }
            ToOverlordMessage::DeletePost(id) => {
                self.delete(id).await?;
            }
            ToOverlordMessage::DeletePriv => {
                GLOBALS.signer.delete_identity();
                Self::delegation_reset().await?;
                GLOBALS
                    .status_queue
                    .write()
                    .write("Identity deleted.".to_string());
            }
            ToOverlordMessage::DeletePub => {
                GLOBALS.signer.clear_public_key();
                Self::delegation_reset().await?;
                GLOBALS.signer.save().await?;
            }
            ToOverlordMessage::DropRelay(relay_url) => {
                let _ = self.to_minions.send(ToMinionMessage {
                    target: relay_url.0,
                    payload: ToMinionPayload {
                        job_id: 0,
                        detail: ToMinionPayloadDetail::Shutdown,
                    },
                });
            }
            ToOverlordMessage::FetchEvent(id, relay_urls) => {
                // Don't do this if we already have the event
                if GLOBALS.storage.has_event(id)? {
                    return Ok(true);
                }

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
            ToOverlordMessage::FollowPubkeyAndRelay(pubkeystr, relay) => {
                self.follow_pubkey_and_relay(pubkeystr, relay).await?;
            }
            ToOverlordMessage::FollowNip05(nip05) => {
                std::mem::drop(tokio::spawn(async move {
                    if let Err(e) = crate::nip05::get_and_follow_nip05(nip05).await {
                        tracing::error!("{}", e);
                    }
                }));
            }
            ToOverlordMessage::FollowNprofile(nprofile) => {
                match Profile::try_from_bech32_string(&nprofile, true) {
                    Ok(np) => self.follow_nprofile(np).await?,
                    Err(e) => GLOBALS.status_queue.write().write(format!("{}", e)),
                }
            }
            ToOverlordMessage::GeneratePrivateKey(mut password) => {
                GLOBALS.signer.generate_private_key(&password)?;
                password.zeroize();
                GLOBALS.signer.save().await?;
            }
            ToOverlordMessage::HideOrShowRelay(relay_url, hidden) => {
                if let Some(mut relay) = GLOBALS.storage.read_relay(&relay_url)? {
                    relay.hidden = hidden;
                    GLOBALS.storage.write_relay(&relay, None)?;
                }
            }
            ToOverlordMessage::ImportPriv(mut import_priv, mut password) => {
                if import_priv.starts_with("ncryptsec") {
                    let epk = EncryptedPrivateKey(import_priv);
                    GLOBALS.signer.set_encrypted_private_key(epk);
                    GLOBALS.signer.unlock_encrypted_private_key(&password)?;
                    password.zeroize();
                    GLOBALS.signer.save().await?;
                } else {
                    let maybe_pk1 = PrivateKey::try_from_bech32_string(&import_priv);
                    let maybe_pk2 = PrivateKey::try_from_hex_string(&import_priv);
                    import_priv.zeroize();
                    if maybe_pk1.is_err() && maybe_pk2.is_err() {
                        password.zeroize();
                        GLOBALS
                            .status_queue
                            .write()
                            .write("Private key not recognized.".to_owned());
                    } else {
                        let privkey = maybe_pk1.unwrap_or_else(|_| maybe_pk2.unwrap());
                        GLOBALS.signer.set_private_key(privkey, &password)?;
                        password.zeroize();
                        GLOBALS.signer.save().await?;
                    }
                }
            }
            ToOverlordMessage::ImportPub(pubstr) => {
                let maybe_pk1 = PublicKey::try_from_bech32_string(&pubstr, true);
                let maybe_pk2 = PublicKey::try_from_hex_string(&pubstr, true);
                if maybe_pk1.is_err() && maybe_pk2.is_err() {
                    GLOBALS
                        .status_queue
                        .write()
                        .write("Public key not recognized.".to_owned());
                } else {
                    let pubkey = maybe_pk1.unwrap_or_else(|_| maybe_pk2.unwrap());
                    GLOBALS.signer.set_public_key(pubkey);
                    GLOBALS.signer.save().await?;
                }
            }
            ToOverlordMessage::Like(id, pubkey) => {
                self.post_like(id, pubkey).await?;
            }
            ToOverlordMessage::MinionIsReady => {
                // currently ignored
            }
            ToOverlordMessage::MinionJobComplete(url, job_id) => {
                // Complete the job if not persistent
                if job_id != 0 {
                    if let Some(mut refmut) = GLOBALS.connected_relays.get_mut(&url) {
                        refmut
                            .value_mut()
                            .retain(|job| job.payload.job_id != job_id);
                    }
                    self.maybe_disconnect_relay(&url)?;
                }
            }
            ToOverlordMessage::MinionJobUpdated(url, old_job_id, new_job_id) => {
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
                    self.maybe_disconnect_relay(&url)?;
                }
            }
            ToOverlordMessage::PickRelays => {
                // When manually doing this, we refresh person_relay scores first which
                // often change if the user just added follows.
                GLOBALS.relay_picker.refresh_person_relay_scores().await?;

                // Then pick
                self.pick_relays().await;
            }
            ToOverlordMessage::PruneCache => {
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
            }
            ToOverlordMessage::PruneDatabase => {
                GLOBALS
                    .status_queue
                    .write()
                    .write("Pruning database, please be patient..".to_owned());

                let now = Unixtime::now().unwrap();
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
            }
            ToOverlordMessage::Post(content, tags, reply_to, dm_channel) => {
                self.post(content, tags, reply_to, dm_channel).await?;
            }
            ToOverlordMessage::PullFollow => {
                self.pull_following().await?;
            }
            ToOverlordMessage::PushFollow => {
                self.push_following().await?;
            }
            ToOverlordMessage::PushMetadata(metadata) => {
                self.push_metadata(metadata).await?;
            }
            ToOverlordMessage::RankRelay(relay_url, rank) => {
                if let Some(mut relay) = GLOBALS.storage.read_relay(&relay_url)? {
                    relay.rank = rank as u64;
                    GLOBALS.storage.write_relay(&relay, None)?;
                }
            }
            ToOverlordMessage::ReengageMinion(url, persistent_jobs) => {
                self.engage_minion(url, persistent_jobs).await?;
            }
            ToOverlordMessage::RefreshFollowedMetadata => {
                self.refresh_followed_metadata().await?;
            }
            ToOverlordMessage::Repost(id) => {
                self.repost(id).await?;
            }
            ToOverlordMessage::Search(text) => {
                Overlord::search(text).await?;
            }
            ToOverlordMessage::SetActivePerson(pubkey) => {
                GLOBALS.people.set_active_person(pubkey).await?;
            }
            ToOverlordMessage::SetThreadFeed(id, referenced_by, relays, author) => {
                self.set_thread_feed(id, referenced_by, relays, author)
                    .await?;
            }
            ToOverlordMessage::Shutdown => {
                tracing::info!("Overlord shutting down");
                return Ok(false);
            }
            ToOverlordMessage::UnlockKey(mut password) => {
                if let Err(e) = GLOBALS.signer.unlock_encrypted_private_key(&password) {
                    tracing::error!("{}", e);
                    GLOBALS
                        .status_queue
                        .write()
                        .write("Could not decrypt key with that password.".to_owned());
                };
                password.zeroize();

                // Update public key from private key
                let public_key = GLOBALS.signer.public_key().unwrap();
                GLOBALS
                    .storage
                    .write_setting_public_key(&Some(public_key), None)?;
            }
            ToOverlordMessage::UpdateFollowing(merge) => {
                self.update_following(merge).await?;
            }
            ToOverlordMessage::UpdateMetadata(pubkey) => {
                let best_relays = GLOBALS.storage.get_best_relays(pubkey, Direction::Write)?;
                let num_relays_per_person = GLOBALS.storage.read_setting_num_relays_per_person();

                // we do 1 more than num_relays_per_person, which is really for main posts,
                // since metadata is more important and I didn't want to bother with
                // another setting.
                for (relay_url, _score) in
                    best_relays.iter().take(num_relays_per_person as usize + 1)
                {
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
            }
            ToOverlordMessage::UpdateMetadataInBulk(mut pubkeys) => {
                let num_relays_per_person = GLOBALS.storage.read_setting_num_relays_per_person();
                let mut map: HashMap<RelayUrl, Vec<PublicKey>> = HashMap::new();
                for pubkey in pubkeys.drain(..) {
                    let best_relays = GLOBALS.storage.get_best_relays(pubkey, Direction::Write)?;
                    for (relay_url, _score) in
                        best_relays.iter().take(num_relays_per_person as usize + 1)
                    {
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
            }
            ToOverlordMessage::VisibleNotesChanged(visible) => {
                let visible: Vec<IdHex> = visible.iter().map(|i| (*i).into()).collect();

                let mut persistent_relay_urls: Vec<RelayUrl> = GLOBALS
                    .connected_relays
                    .iter()
                    .filter_map(|r| {
                        for job in r.value() {
                            if job.reason.persistent() {
                                return Some(r.key().clone());
                            }
                        }
                        None
                    })
                    .collect();

                // Resubscribe to augments on all relays that have
                // any feed-event subscriptions (see filter above)
                for url in persistent_relay_urls.drain(..) {
                    self.engage_minion(
                        url,
                        vec![RelayJob {
                            reason: RelayConnectionReason::FetchAugments,
                            payload: ToMinionPayload {
                                job_id: rand::random::<u64>(),
                                detail: ToMinionPayloadDetail::SubscribeAugments(visible.clone()),
                            },
                        }],
                    )
                    .await?;
                }
            }
            ToOverlordMessage::ZapStart(id, pubkey, lnurl) => {
                self.zap_start(id, pubkey, lnurl).await?;
            }
            ToOverlordMessage::Zap(id, pubkey, msats, comment) => {
                self.zap(id, pubkey, msats, comment).await?;
            }
        }

        Ok(true)
    }

    fn maybe_disconnect_relay(&mut self, url: &RelayUrl) -> Result<(), Error> {
        if let Some(refmut) = GLOBALS.connected_relays.get_mut(url) {
            // If no job remains, disconnect the relay
            let mut disconnect = refmut.value().is_empty();

            // If only one 'augments' job remains, disconnect the relay
            if refmut.value().len() == 1
                && refmut.value()[0].reason == RelayConnectionReason::FetchAugments
            {
                disconnect = true;
            }

            if disconnect {
                let _ = self.to_minions.send(ToMinionMessage {
                    target: url.0.clone(),
                    payload: ToMinionPayload {
                        job_id: 0,
                        detail: ToMinionPayloadDetail::Shutdown,
                    },
                });
            }
        }

        Ok(())
    }

    async fn follow_pubkey_and_relay(
        &mut self,
        pubkeystr: String,
        relay: RelayUrl,
    ) -> Result<(), Error> {
        let pubkey = match PublicKey::try_from_bech32_string(&pubkeystr, true) {
            Ok(pk) => pk,
            Err(_) => PublicKey::try_from_hex_string(&pubkeystr, true)?,
        };
        GLOBALS.people.follow(&pubkey, true)?;

        tracing::debug!("Followed {}", &pubkey.as_hex_string());

        // Save relay
        let db_relay = Relay::new(relay.clone());
        GLOBALS.storage.write_relay(&db_relay, None)?;

        let now = Unixtime::now().unwrap().0 as u64;

        // Save person_relay
        let mut pr = match GLOBALS.storage.read_person_relay(pubkey, &relay)? {
            Some(pr) => pr,
            None => PersonRelay::new(pubkey, relay.clone()),
        };
        pr.last_suggested_kind3 = Some(now);
        pr.manually_paired_read = true;
        pr.manually_paired_write = true;
        GLOBALS.storage.write_person_relay(&pr, None)?;

        // async_follow added them to the relay tracker.
        // Pick relays to start tracking them now
        self.pick_relays().await;

        tracing::info!("Setup 1 relay for {}", &pubkey.as_hex_string());

        Ok(())
    }

    async fn post(
        &mut self,
        content: String,
        mut tags: Vec<Tag>,
        reply_to: Option<Id>,
        dm_channel: Option<DmChannel>,
    ) -> Result<(), Error> {
        let public_key = match GLOBALS.signer.public_key() {
            Some(pk) => pk,
            None => {
                tracing::warn!("No public key! Not posting");
                return Ok(());
            }
        };

        let pre_event = match dm_channel {
            Some(dmc) => {
                if dmc.keys().len() > 1 {
                    return Err((ErrorKind::GroupDmsNotYetSupported, file!(), line!()).into());
                }

                let recipient = if dmc.keys().is_empty() {
                    public_key // must be to yourself
                } else {
                    dmc.keys()[0]
                };

                // On a DM, we ignore tags and reply_to

                GLOBALS.signer.new_nip04(recipient, &content)?
            }
            _ => {
                if GLOBALS.storage.read_setting_set_client_tag() {
                    tags.push(Tag::Other {
                        tag: "client".to_owned(),
                        data: vec!["gossip".to_owned()],
                    });
                }

                // Add Tags based on references in the content
                //
                // FIXME - this function takes a 'tags' variable. We may want to let
                // the user determine which tags to keep and which to delete, so we
                // should probably move this processing into the post editor instead.
                // For now, I'm just trying to remove the old #[0] type substitutions
                // and use the new NostrBech32 parsing.
                for bech32 in NostrBech32::find_all_in_string(&content).iter() {
                    match bech32 {
                        NostrBech32::EventAddr(ea) => {
                            add_addr_to_tags(
                                &mut tags,
                                ea.kind,
                                ea.author.into(),
                                ea.d.clone(),
                                ea.relays.get(0).cloned(),
                            )
                            .await;
                        }
                        NostrBech32::EventPointer(ep) => {
                            // NIP-10: "Those marked with "mention" denote a quoted or reposted event id."
                            add_event_to_tags(&mut tags, ep.id, "mention").await;
                        }
                        NostrBech32::Id(id) => {
                            // NIP-10: "Those marked with "mention" denote a quoted or reposted event id."
                            add_event_to_tags(&mut tags, *id, "mention").await;
                        }
                        NostrBech32::Profile(prof) => {
                            if dm_channel.is_none() {
                                add_pubkey_to_tags(&mut tags, &prof.pubkey).await;
                            }
                        }
                        NostrBech32::Pubkey(pk) => {
                            if dm_channel.is_none() {
                                add_pubkey_to_tags(&mut tags, pk).await;
                            }
                        }
                        NostrBech32::Relay(_) => {
                            // we don't need to add this to tags I don't think.
                        }
                    }
                }

                // Standardize nostr links (prepend 'nostr:' where missing)
                // (This was a bad idea to do this late in the process, it breaks links that contain
                //  nostr urls)
                // content = NostrUrl::urlize(&content);

                // Find and tag all hashtags
                for capture in GLOBALS.hashtag_regex.captures_iter(&content) {
                    tags.push(Tag::Hashtag {
                        hashtag: capture[1][1..].to_string(),
                        trailing: Vec::new(),
                    });
                }

                if let Some(parent_id) = reply_to {
                    // Get the event we are replying to
                    let parent = match GLOBALS.storage.read_event(parent_id)? {
                        Some(e) => e,
                        None => return Err("Cannot find event we are replying to.".into()),
                    };

                    // Add a 'p' tag for the author we are replying to (except if it is our own key)
                    if parent.pubkey != public_key {
                        if dm_channel.is_none() {
                            add_pubkey_to_tags(&mut tags, &parent.pubkey).await;
                        }
                    }

                    // Add all the 'p' tags from the note we are replying to (except our own)
                    // FIXME: Should we avoid taging people who are muted?
                    if dm_channel.is_none() {
                        for tag in &parent.tags {
                            if let Tag::Pubkey { pubkey, .. } = tag {
                                if pubkey.as_str() != public_key.as_hex_string() {
                                    add_pubkey_hex_to_tags(&mut tags, pubkey).await;
                                }
                            }
                        }
                    }

                    if let Some((root, _maybeurl)) = parent.replies_to_root() {
                        // Add an 'e' tag for the root
                        add_event_to_tags(&mut tags, root, "root").await;

                        // Add an 'e' tag for the note we are replying to
                        add_event_to_tags(&mut tags, parent_id, "reply").await;
                    } else {
                        let ancestors = parent.referred_events();
                        if ancestors.is_empty() {
                            // parent is the root
                            add_event_to_tags(&mut tags, parent_id, "root").await;
                        } else {
                            // Add an 'e' tag for the note we are replying to
                            // (and we don't know about the root, the parent is malformed).
                            add_event_to_tags(&mut tags, parent_id, "reply").await;
                        }
                    }

                    // Possibly propagate a subject tag
                    for tag in &parent.tags {
                        if let Tag::Subject { subject, .. } = tag {
                            let mut subject = subject.to_owned();
                            if !subject.starts_with("Re: ") {
                                subject = format!("Re: {}", subject);
                            }
                            subject = subject.chars().take(80).collect();
                            add_subject_to_tags_if_missing(&mut tags, subject);
                        }
                    }
                }

                PreEvent {
                    pubkey: public_key,
                    created_at: Unixtime::now().unwrap(),
                    kind: EventKind::TextNote,
                    tags,
                    content,
                    ots: None,
                }
            }
        };

        // Copy the tagged pubkeys for determine which relays to send to
        let mut tagged_pubkeys: Vec<PublicKey> = pre_event
            .tags
            .iter()
            .filter_map(|t| {
                if let Tag::Pubkey { pubkey, .. } = t {
                    match PublicKey::try_from_hex_string(pubkey, true) {
                        Ok(pk) => Some(pk),
                        _ => None,
                    }
                } else {
                    None
                }
            })
            .collect();

        let event = {
            let powint = GLOBALS.storage.read_setting_pow();
            let pow = if powint > 0 { Some(powint) } else { None };
            let (work_sender, work_receiver) = mpsc::channel();

            std::thread::spawn(move || {
                work_logger(work_receiver, powint);
            });

            GLOBALS
                .signer
                .sign_preevent(pre_event, pow, Some(work_sender))?
        };

        // Process this event locally
        crate::process::process_new_event(&event, None, None, false, false).await?;

        // Determine which relays to post this to
        let mut relay_urls: Vec<RelayUrl> = Vec::new();
        {
            // Get 'read' relays for everybody tagged in the event.
            // Currently we take the 2 best read relays per person
            for pubkey in tagged_pubkeys.drain(..) {
                let best_relays: Vec<RelayUrl> = GLOBALS
                    .storage
                    .get_best_relays(pubkey, Direction::Read)?
                    .drain(..)
                    .take(2)
                    .map(|(u, _)| u)
                    .collect();
                relay_urls.extend(best_relays);
            }

            // Get all of the relays that we write to
            let write_relay_urls: Vec<RelayUrl> = GLOBALS
                .storage
                .filter_relays(|r| r.has_usage_bits(Relay::WRITE))?
                .iter()
                .map(|relay| relay.url.clone())
                .collect();
            relay_urls.extend(write_relay_urls);

            relay_urls.sort();
            relay_urls.dedup();
        }

        for url in relay_urls {
            // Send it the event to post
            tracing::debug!("Asking {} to post", &url);

            self.engage_minion(
                url.clone(),
                vec![RelayJob {
                    reason: RelayConnectionReason::PostEvent,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvent(Box::new(event.clone())),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    async fn advertise_relay_list(&mut self) -> Result<(), Error> {
        let public_key = match GLOBALS.signer.public_key() {
            Some(pk) => pk,
            None => {
                tracing::warn!("No public key! Not posting");
                return Ok(());
            }
        };

        let inbox_or_outbox_relays: Vec<Relay> = GLOBALS
            .storage
            .filter_relays(|r| r.has_usage_bits(Relay::INBOX) || r.has_usage_bits(Relay::OUTBOX))?;
        let mut tags: Vec<Tag> = Vec::new();
        for relay in inbox_or_outbox_relays.iter() {
            tags.push(Tag::Reference {
                url: relay.url.to_unchecked_url(),
                marker: if relay.has_usage_bits(Relay::INBOX) && relay.has_usage_bits(Relay::OUTBOX)
                {
                    None
                } else if relay.has_usage_bits(Relay::INBOX) {
                    Some("read".to_owned()) // NIP-65 uses the term 'read' instead of 'inbox'
                } else if relay.has_usage_bits(Relay::OUTBOX) {
                    Some("write".to_owned()) // NIP-65 uses the term 'write' instead of 'outbox'
                } else {
                    unreachable!()
                },
                trailing: Vec::new(),
            });
        }

        let pre_event = PreEvent {
            pubkey: public_key,
            created_at: Unixtime::now().unwrap(),
            kind: EventKind::RelayList,
            tags,
            content: "".to_string(),
            ots: None,
        };

        let event = GLOBALS.signer.sign_preevent(pre_event, None, None)?;

        let advertise_to_relay_urls: Vec<RelayUrl> = GLOBALS
            .storage
            .filter_relays(|r| r.has_usage_bits(Relay::ADVERTISE))?
            .iter()
            .map(|relay| relay.url.clone())
            .collect();

        for relay_url in advertise_to_relay_urls {
            // Send it the event to post
            tracing::debug!("Asking {} to post", &relay_url);

            self.engage_minion(
                relay_url.to_owned(),
                vec![RelayJob {
                    reason: RelayConnectionReason::Advertising,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvent(Box::new(event.clone())),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    async fn post_like(&mut self, id: Id, pubkey: PublicKey) -> Result<(), Error> {
        let event = {
            let public_key = match GLOBALS.signer.public_key() {
                Some(pk) => pk,
                None => {
                    tracing::warn!("No public key! Not posting");
                    return Ok(());
                }
            };

            let mut tags: Vec<Tag> = vec![
                Tag::Event {
                    id,
                    recommended_relay_url: Relay::recommended_relay_for_reply(id)
                        .await?
                        .map(|rr| rr.to_unchecked_url()),
                    marker: None,
                    trailing: Vec::new(),
                },
                Tag::Pubkey {
                    pubkey: pubkey.into(),
                    recommended_relay_url: None,
                    petname: None,
                    trailing: Vec::new(),
                },
            ];

            if GLOBALS.storage.read_setting_set_client_tag() {
                tags.push(Tag::Other {
                    tag: "client".to_owned(),
                    data: vec!["gossip".to_owned()],
                });
            }

            let pre_event = PreEvent {
                pubkey: public_key,
                created_at: Unixtime::now().unwrap(),
                kind: EventKind::Reaction,
                tags,
                content: "+".to_owned(),
                ots: None,
            };

            let powint = GLOBALS.storage.read_setting_pow();
            let pow = if powint > 0 { Some(powint) } else { None };
            let (work_sender, work_receiver) = mpsc::channel();

            std::thread::spawn(move || {
                work_logger(work_receiver, powint);
            });

            GLOBALS
                .signer
                .sign_preevent(pre_event, pow, Some(work_sender))?
        };

        let relays: Vec<Relay> = GLOBALS
            .storage
            .filter_relays(|r| r.has_usage_bits(Relay::WRITE))?;
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
                        detail: ToMinionPayloadDetail::PostEvent(Box::new(event.clone())),
                    },
                }],
            )
            .await?;
        }

        // Process the message for ourself
        crate::process::process_new_event(&event, None, None, false, false).await?;

        Ok(())
    }

    async fn pull_following(&mut self) -> Result<(), Error> {
        // Pull our list from all of the relays we post to
        let relays: Vec<Relay> = GLOBALS
            .storage
            .filter_relays(|r| r.has_usage_bits(Relay::WRITE))?;

        for relay in relays {
            // Send it the event to pull our followers
            tracing::debug!("Asking {} to pull our followers", &relay.url);

            self.engage_minion(
                relay.url.clone(),
                vec![RelayJob {
                    reason: RelayConnectionReason::FetchContacts,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PullFollowing,
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    async fn push_following(&mut self) -> Result<(), Error> {
        let event = GLOBALS.people.generate_contact_list_event().await?;

        // Push to all of the relays we post to
        let relays: Vec<Relay> = GLOBALS
            .storage
            .filter_relays(|r| r.has_usage_bits(Relay::WRITE))?;

        for relay in relays {
            // Send it the event to pull our followers
            tracing::debug!("Pushing ContactList to {}", &relay.url);

            self.engage_minion(
                relay.url.clone(),
                vec![RelayJob {
                    reason: RelayConnectionReason::PostContacts,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvent(Box::new(event.clone())),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    async fn clear_following(&mut self) -> Result<(), Error> {
        GLOBALS.people.follow_none()?;
        Ok(())
    }

    async fn push_metadata(&mut self, metadata: Metadata) -> Result<(), Error> {
        let public_key = match GLOBALS.signer.public_key() {
            Some(pk) => pk,
            None => return Err((ErrorKind::NoPrivateKey, file!(), line!()).into()), // not even a public key
        };

        let pre_event = PreEvent {
            pubkey: public_key,
            created_at: Unixtime::now().unwrap(),
            kind: EventKind::Metadata,
            tags: vec![],
            content: serde_json::to_string(&metadata)?,
            ots: None,
        };

        let event = GLOBALS.signer.sign_preevent(pre_event, None, None)?;

        // Push to all of the relays we post to
        let relays: Vec<Relay> = GLOBALS
            .storage
            .filter_relays(|r| r.has_usage_bits(Relay::WRITE))?;

        for relay in relays {
            // Send it the event to pull our followers
            tracing::debug!("Pushing Metadata to {}", &relay.url);

            self.engage_minion(
                relay.url.clone(),
                vec![RelayJob {
                    reason: RelayConnectionReason::PostMetadata,
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvent(Box::new(event.clone())),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    // This gets it whether we had it or not. Because it might have changed.
    async fn refresh_followed_metadata(&mut self) -> Result<(), Error> {
        let mut pubkeys = GLOBALS.people.get_followed_pubkeys();

        // add own pubkey as well
        if let Some(pubkey) = GLOBALS.signer.public_key() {
            pubkeys.push(pubkey)
        }

        let num_relays_per_person = GLOBALS.storage.read_setting_num_relays_per_person();

        let mut map: HashMap<RelayUrl, Vec<PublicKey>> = HashMap::new();

        // Sort the people into the relays we will find their metadata at
        for pubkey in &pubkeys {
            for relayscore in GLOBALS
                .storage
                .get_best_relays(*pubkey, Direction::Write)?
                .drain(..)
                .take(num_relays_per_person as usize)
            {
                map.entry(relayscore.0)
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

    async fn repost(&mut self, id: Id) -> Result<(), Error> {
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

        let mut tags: Vec<Tag> = vec![
            Tag::Event {
                id,
                recommended_relay_url: {
                    let seen_on = GLOBALS.storage.get_event_seen_on_relay(reposted_event.id)?;
                    if seen_on.is_empty() {
                        Relay::recommended_relay_for_reply(id)
                            .await?
                            .map(|rr| rr.to_unchecked_url())
                    } else {
                        seen_on.get(0).map(|(rurl, _)| rurl.to_unchecked_url())
                    }
                },
                marker: None,
                trailing: Vec::new(),
            },
            Tag::Pubkey {
                pubkey: reposted_event.pubkey.into(),
                recommended_relay_url: None,
                petname: None,
                trailing: Vec::new(),
            },
        ];

        let event = {
            let public_key = match GLOBALS.signer.public_key() {
                Some(pk) => pk,
                None => {
                    tracing::warn!("No public key! Not posting");
                    return Ok(());
                }
            };

            if GLOBALS.storage.read_setting_set_client_tag() {
                tags.push(Tag::Other {
                    tag: "client".to_owned(),
                    data: vec!["gossip".to_owned()],
                });
            }

            let pre_event = PreEvent {
                pubkey: public_key,
                created_at: Unixtime::now().unwrap(),
                kind: EventKind::Repost,
                tags,
                content: serde_json::to_string(&reposted_event)?,
                ots: None,
            };

            let powint = GLOBALS.storage.read_setting_pow();
            let pow = if powint > 0 { Some(powint) } else { None };
            let (work_sender, work_receiver) = mpsc::channel();

            std::thread::spawn(move || {
                work_logger(work_receiver, powint);
            });

            GLOBALS
                .signer
                .sign_preevent(pre_event, pow, Some(work_sender))?
        };

        // Process this event locally
        crate::process::process_new_event(&event, None, None, false, false).await?;

        // Determine which relays to post this to
        let mut relay_urls: Vec<RelayUrl> = Vec::new();
        {
            // Get all of the relays that we write to
            let write_relay_urls: Vec<RelayUrl> = GLOBALS
                .storage
                .filter_relays(|r| r.has_usage_bits(Relay::WRITE))?
                .iter()
                .map(|relay| relay.url.clone())
                .collect();
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
                        detail: ToMinionPayloadDetail::PostEvent(Box::new(event.clone())),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    async fn set_thread_feed(
        &mut self,
        id: Id,
        referenced_by: Id,
        mut relays: Vec<RelayUrl>,
        author: Option<PublicKey>,
    ) -> Result<(), Error> {
        // We are responsible for loading all the ancestors and all the replies, and
        // process.rs is responsible for building the relationships.
        // The UI can only show events if they are loaded into memory and the relationships
        // exist in memory.

        // Our task is fourfold:
        //   ancestors from sqlite, replies from sqlite
        //   ancestors from relays, replies from relays,

        // We simplify things by asking for this data from every relay we are
        // connected to, as well as any relays we discover might know.  This is
        // more than strictly necessary, but not too expensive.

        let mut missing_ancestors: Vec<Id> = Vec::new();

        // Include the relays where the referenced_by event was seen
        relays.extend(
            GLOBALS
                .storage
                .get_event_seen_on_relay(referenced_by)?
                .drain(..)
                .map(|(url, _time)| url),
        );
        relays.extend(
            GLOBALS
                .storage
                .get_event_seen_on_relay(id)?
                .drain(..)
                .map(|(url, _time)| url),
        );

        // If we have less than 2 relays, include the write relays of the author
        if relays.len() < 2 {
            if let Some(pk) = author {
                let author_relays: Vec<RelayUrl> = GLOBALS
                    .storage
                    .get_best_relays(pk, Direction::Write)?
                    .drain(..)
                    .map(|pair| pair.0)
                    .collect();
                relays.extend(author_relays);
            }
        }

        // Climb the tree as high as we can, and if there are higher events,
        // we will ask for those in the initial subscription
        let highest_parent_id =
            if let Some(hpid) = GLOBALS.storage.get_highest_local_parent_event_id(id)? {
                hpid
            } else {
                // we don't have the event itself!
                missing_ancestors.push(id);
                id
            };

        // Set the thread feed to the highest parent that we have, or to the event itself
        // even if we don't have it (it might be coming in soon)
        GLOBALS.feed.set_thread_parent(highest_parent_id);

        // Collect missing ancestors and potential relays further up the chain
        if let Some(highest_parent) = GLOBALS.storage.read_event(highest_parent_id)? {
            // Use relays in 'e' tags
            for (id, opturl, _marker) in highest_parent.referred_events() {
                missing_ancestors.push(id);
                if let Some(url) = opturl {
                    relays.push(url);
                }
            }

            // fiatjaf's suggestion from issue #187, use 'p' tag url mentions too, since
            // those people probably wrote the ancestor events so probably on those
            // relays
            for (_pk, opturl, _nick) in highest_parent.people() {
                if let Some(url) = opturl {
                    relays.push(url);
                }
            }
        }

        let missing_ancestors_hex: Vec<IdHex> =
            missing_ancestors.iter().map(|id| (*id).into()).collect();

        // Subscribe on relays
        if relays.is_empty() {
            GLOBALS
                .status_queue
                .write()
                .write("Could not find any relays for that event".to_owned());
            return Ok(());
        } else {
            // Clean up relays
            relays.sort();
            relays.dedup();

            // Cancel current thread subscriptions, if any
            let _ = self.to_minions.send(ToMinionMessage {
                target: "all".to_string(),
                payload: ToMinionPayload {
                    job_id: 0,
                    detail: ToMinionPayloadDetail::UnsubscribeThreadFeed,
                },
            });

            for url in relays.iter() {
                // Subscribe
                self.engage_minion(
                    url.to_owned(),
                    vec![RelayJob {
                        reason: RelayConnectionReason::ReadThread,
                        payload: ToMinionPayload {
                            job_id: rand::random::<u64>(),
                            detail: ToMinionPayloadDetail::SubscribeThreadFeed(
                                id.into(),
                                missing_ancestors_hex.clone(),
                            ),
                        },
                    }],
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn follow_nprofile(&mut self, nprofile: Profile) -> Result<(), Error> {
        GLOBALS.people.follow(&nprofile.pubkey, true)?;

        // Set their relays
        for relay in nprofile.relays.iter() {
            if let Ok(relay_url) = RelayUrl::try_from_unchecked_url(relay) {
                // Save relay
                let db_relay = Relay::new(relay_url.clone());
                GLOBALS.storage.write_relay(&db_relay, None)?;

                // Save person_relay
                let mut pr = match GLOBALS
                    .storage
                    .read_person_relay(nprofile.pubkey, &relay_url)?
                {
                    Some(pr) => pr,
                    None => PersonRelay::new(nprofile.pubkey, relay_url.clone()),
                };
                pr.last_suggested_nip05 = Some(Unixtime::now().unwrap().0 as u64);
                GLOBALS.storage.write_person_relay(&pr, None)?;
            }
        }

        GLOBALS
            .status_queue
            .write()
            .write(format!("Followed user at {} relays", nprofile.relays.len()));

        // async_follow added them to the relay tracker.
        // Pick relays to start tracking them now
        self.pick_relays().await;

        Ok(())
    }

    async fn delegation_reset() -> Result<(), Error> {
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

    async fn delete(&mut self, id: Id) -> Result<(), Error> {
        let tags: Vec<Tag> = vec![Tag::Event {
            id,
            recommended_relay_url: None,
            marker: None,
            trailing: Vec::new(),
        }];

        let event = {
            let public_key = match GLOBALS.signer.public_key() {
                Some(pk) => pk,
                None => {
                    tracing::warn!("No public key! Not posting");
                    return Ok(());
                }
            };

            let pre_event = PreEvent {
                pubkey: public_key,
                created_at: Unixtime::now().unwrap(),
                kind: EventKind::EventDeletion,
                tags,
                content: "".to_owned(), // FIXME, option to supply a delete reason
                ots: None,
            };

            // Should we add a pow? Maybe the relay needs it.
            GLOBALS.signer.sign_preevent(pre_event, None, None)?
        };

        // Process this event locally
        crate::process::process_new_event(&event, None, None, false, false).await?;

        // Determine which relays to post this to
        let mut relay_urls: Vec<RelayUrl> = Vec::new();
        {
            // Get all of the relays that we write to
            let write_relay_urls: Vec<RelayUrl> = GLOBALS
                .storage
                .filter_relays(|r| r.has_usage_bits(Relay::WRITE))?
                .iter()
                .map(|relay| relay.url.clone())
                .collect();
            relay_urls.extend(write_relay_urls);
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
                        detail: ToMinionPayloadDetail::PostEvent(Box::new(event.clone())),
                    },
                }],
            )
            .await?;
        }

        Ok(())
    }

    // This updates the actual following list (based on the follow flag in the person table)
    // from the last ContactList received
    async fn update_following(&mut self, merge: bool) -> Result<(), Error> {
        // Load the latest contact list from the database
        let our_contact_list = {
            let pubkey = match GLOBALS.signer.public_key() {
                Some(pk) => pk,
                None => return Ok(()), // we cannot do anything without an identity setup first
            };

            if let Some(event) = GLOBALS.storage.fetch_contact_list(&pubkey)? {
                event.clone()
            } else {
                return Ok(()); // we have no contact list to update from
            }
        };

        let mut pubkeys: Vec<PublicKey> = Vec::new();

        let now = Unixtime::now().unwrap();

        // 'p' tags represent the author's contacts
        for tag in &our_contact_list.tags {
            if let Tag::Pubkey {
                pubkey,
                recommended_relay_url,
                ..
            } = tag
            {
                if let Ok(pubkey) = PublicKey::try_from_hex_string(pubkey, true) {
                    // Make sure we have that person
                    GLOBALS.people.create_all_if_missing(&[pubkey.to_owned()])?;

                    // Save the pubkey for actual following them (outside of the loop in a batch)
                    pubkeys.push(pubkey.to_owned());

                    // If there is a URL
                    if let Some(url) = recommended_relay_url
                        .as_ref()
                        .and_then(|rru| RelayUrl::try_from_unchecked_url(rru).ok())
                    {
                        // Save relay if missing
                        GLOBALS.storage.write_relay_if_missing(&url, None)?;

                        // create or update person_relay last_suggested_kind3
                        let mut pr = match GLOBALS.storage.read_person_relay(pubkey, &url)? {
                            Some(pr) => pr,
                            None => PersonRelay::new(pubkey, url.clone()),
                        };
                        pr.last_suggested_kind3 = Some(now.0 as u64);
                        GLOBALS.storage.write_person_relay(&pr, None)?;
                    }
                    // TBD: do something with the petname
                }
            }
        }

        // Follow all those pubkeys, and unfollow everbody else if merge=false
        GLOBALS.people.follow_all(&pubkeys, merge)?;

        // Update last_contact_list_edit
        let last_edit = if merge {
            Unixtime::now().unwrap() // now, since superior to the last event
        } else {
            our_contact_list.created_at
        };
        GLOBALS
            .storage
            .write_last_contact_list_edit(last_edit.0, None)?;

        // Pick relays again
        {
            // Refresh person-relay scores
            GLOBALS.relay_picker.refresh_person_relay_scores().await?;

            // Then pick
            self.pick_relays().await;
        }

        Ok(())
    }

    async fn search(mut text: String) -> Result<(), Error> {
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
                    if let Some(event) = GLOBALS
                        .storage
                        .find_events(
                            &[ea.kind],
                            &[ea.author],
                            None,
                            |event| {
                                event.tags.iter().any(|tag| {
                                    if let Tag::Identifier { d, .. } = tag {
                                        if *d == ea.d {
                                            return true;
                                        }
                                    }
                                    false
                                })
                            },
                            true,
                        )?
                        .get(1)
                    {
                        note_search_results.push(event.clone());
                    }
                }
                NostrBech32::EventPointer(ep) => {
                    if let Some(event) = GLOBALS.storage.read_event(ep.id)? {
                        note_search_results.push(event);
                    }
                }
                NostrBech32::Id(id) => {
                    if let Some(event) = GLOBALS.storage.read_event(id)? {
                        note_search_results.push(event);
                    }
                }
                NostrBech32::Profile(prof) => {
                    if let Some(person) = GLOBALS.storage.read_person(&prof.pubkey)? {
                        people_search_results.push(person);
                    }
                }
                NostrBech32::Pubkey(pk) => {
                    if let Some(person) = GLOBALS.storage.read_person(&pk)? {
                        people_search_results.push(person);
                    }
                }
                NostrBech32::Relay(_relay) => (),
            }
        }

        people_search_results.extend(GLOBALS.storage.filter_people(|p| {
            if let Some(metadata) = &p.metadata {
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
        })?);

        note_search_results.extend(GLOBALS.storage.search_events(&text)?);

        *GLOBALS.people_search_results.write() = people_search_results;
        *GLOBALS.note_search_results.write() = note_search_results;

        Ok(())
    }

    async fn zap_start(
        &mut self,
        id: Id,
        target_pubkey: PublicKey,
        lnurl: UncheckedUrl,
    ) -> Result<(), Error> {
        if GLOBALS.signer.public_key().is_none() {
            tracing::warn!("You need to setup your identity to zap.");
            GLOBALS
                .status_queue
                .write()
                .write("You need to setup your identity to zap.".to_string());
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

    async fn zap(
        &mut self,
        id: Id,
        target_pubkey: PublicKey,
        msats: MilliSatoshi,
        comment: String,
    ) -> Result<(), Error> {
        use serde_json::Value;

        let user_pubkey = match GLOBALS.signer.public_key() {
            Some(pk) => pk,
            None => {
                tracing::warn!("You need to setup your identity to zap.");
                GLOBALS
                    .status_queue
                    .write()
                    .write("You need to setup your identity to zap.".to_string());
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
            let mut target_read_relays = GLOBALS
                .storage
                .get_best_relays(target_pubkey, Direction::Read)?;
            let target_read_relays: Vec<RelayUrl> =
                target_read_relays.drain(..).map(|pair| pair.0).collect();
            relays.extend(target_read_relays);

            // Add all my write relays
            let write_relay_urls: Vec<RelayUrl> = GLOBALS
                .storage
                .filter_relays(|r| r.has_usage_bits(Relay::WRITE))?
                .iter()
                .map(|relay| relay.url.clone())
                .collect();
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

        // Generate the zap request event
        let pre_event = PreEvent {
            pubkey: user_pubkey,
            created_at: Unixtime::now().unwrap(),
            kind: EventKind::ZapRequest,
            tags: vec![
                Tag::Event {
                    id,
                    recommended_relay_url: None,
                    marker: None,
                    trailing: Vec::new(),
                },
                Tag::Pubkey {
                    pubkey: target_pubkey.into(),
                    recommended_relay_url: None,
                    petname: None,
                    trailing: Vec::new(),
                },
                Tag::Other {
                    tag: "relays".to_owned(),
                    data: relays,
                },
                Tag::Other {
                    tag: "amount".to_owned(),
                    data: vec![msats_string.clone()],
                },
                Tag::Other {
                    tag: "lnurl".to_owned(),
                    data: vec![lnurl.as_str().to_owned()],
                },
            ],
            content: comment,
            ots: None,
        };

        let event = GLOBALS.signer.sign_preevent(pre_event, None, None)?;
        let serialized_event = serde_json::to_string(&event)?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::new(15, 0))
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .build()?;

        let mut url = match url::Url::parse(&callback.0) {
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
