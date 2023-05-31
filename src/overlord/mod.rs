mod minion;

use crate::comms::{
    RelayJob, ToMinionMessage, ToMinionPayload, ToMinionPayloadDetail, ToOverlordMessage,
};
use crate::db::{DbEvent, DbEventFlags, DbEventRelay, DbPersonRelay, DbRelay};
use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::people::People;
use crate::tags::{
    add_event_to_tags, add_pubkey_hex_to_tags, add_pubkey_to_tags, add_subject_to_tags_if_missing,
};
use dashmap::mapref::entry::Entry;
use gossip_relay_picker::{Direction, RelayAssignment};
use minion::Minion;
use nostr_types::{
    EncryptedPrivateKey, Event, EventKind, Filter, Id, IdHex, IdHexPrefix, Metadata, NostrBech32,
    NostrUrl, PreEvent, PrivateKey, Profile, PublicKey, PublicKeyHex, RelayUrl, Tag, Unixtime,
};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::{select, task};
use zeroize::Zeroize;

pub struct Overlord {
    to_minions: Sender<ToMinionMessage>,
    inbox: UnboundedReceiver<ToOverlordMessage>,

    // All the minion tasks running.
    minions: task::JoinSet<()>,

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

        tracing::info!("Overlord signalling UI to shutdown");

        GLOBALS.shutting_down.store(true, Ordering::Relaxed);

        tracing::info!("Overlord signalling minions to shutdown");

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
            let task_nextjoined = self.minions.join_next_with_id().await;

            self.handle_task_nextjoined(task_nextjoined).await;
        }

        tracing::info!("Overlord confirms all minions have shutdown");
    }

    pub async fn run_inner(&mut self) -> Result<(), Error> {
        // Load signer from settings
        GLOBALS.signer.load_from_settings().await;

        // FIXME - if this needs doing, it should be done dynamically as
        //         new people are encountered, not batch-style on startup.
        // Create a person record for every person seen

        People::populate_new_people().await?;

        // FIXME - if this needs doing, it should be done dynamically as
        //         new people are encountered, not batch-style on startup.
        // Create a relay record for every relay in person_relay map (these get
        // updated from events without necessarily updating our relays list)
        DbRelay::populate_new_relays().await?;

        // Load relays from the database
        {
            let mut all_relays: Vec<DbRelay> = DbRelay::fetch(None).await?;
            for dbrelay in all_relays.drain(..) {
                GLOBALS.all_relays.insert(dbrelay.url.clone(), dbrelay);
            }
        }

        // Load followed people from the database
        GLOBALS.people.load_all_followed().await?;

        // Load contact list from the database
        if let Some(pk) = GLOBALS.signer.public_key() {
            if let Some(event) = DbEvent::fetch_last_contact_list(pk.into()).await? {
                crate::process::process_new_event(&event, false, None, None).await?;
            }
        }

        // Load last_contact_list_edit
        {
            let db = GLOBALS.db.lock().await;
            if let Ok(last_edit) = db.query_row(
                "SELECT last_contact_list_edit FROM local_settings LIMIT 1",
                [],
                |row| row.get::<usize, i64>(0),
            ) {
                GLOBALS
                    .people
                    .last_contact_list_edit
                    .store(last_edit, Ordering::Relaxed);
            }
        }

        // Load delegation tag
        GLOBALS.delegation.load_through_settings()?;

        // Initialize the relay picker
        GLOBALS.relay_picker.init().await?;

        let now = Unixtime::now().unwrap();

        // Load reply-related events from database and process
        // (where you are tagged)
        {
            let replies_chunk = GLOBALS.settings.read().replies_chunk;
            let then = now.0 - replies_chunk as i64;

            let db_events = DbEvent::fetch_reply_related(then).await?;

            // Map db events into Events
            let mut events: Vec<Event> = Vec::with_capacity(db_events.len());
            for dbevent in db_events.iter() {
                let e = serde_json::from_str(&dbevent.raw)?;
                events.push(e);
            }

            // Process these events
            let mut count = 0;
            for event in events.iter() {
                count += 1;
                crate::process::process_new_event(event, false, None, None).await?;
            }
            tracing::info!("Loaded {} reply related events from the database", count);
        }

        // Load feed-related events from database and process
        {
            let feed_chunk = GLOBALS.settings.read().feed_chunk;
            let then = now.0 - feed_chunk as i64;

            let where_kind = GLOBALS
                .settings
                .read()
                .feed_related_event_kinds()
                .iter()
                .map(|e| <EventKind as Into<u64>>::into(*e))
                .map(|e| e.to_string())
                .collect::<Vec<String>>()
                .join(",");

            let cond = format!(
                " kind in ({}) AND created_at > {} ORDER BY created_at ASC",
                where_kind, then
            );

            let db_events = DbEvent::fetch(Some(&cond)).await?;

            // Map db events into Events
            let mut events: Vec<Event> = Vec::with_capacity(db_events.len());
            for dbevent in db_events.iter() {
                let e = serde_json::from_str(&dbevent.raw)?;
                events.push(e);
            }

            // Process these events
            let mut count = 0;
            for event in events.iter() {
                count += 1;
                crate::process::process_new_event(event, false, None, None).await?;
            }
            tracing::info!("Loaded {} feed related events from the database", count);

            // As soon as we have the feed events loaded, we trigger a feed recompute
            GLOBALS.feed.ready.store(true, Ordering::Relaxed);
            GLOBALS.feed.recompute().await?;
        }

        // Load viewed events set into memory
        for id in DbEventFlags::load_all_viewed().await?.iter() {
            GLOBALS.viewed_events.insert(*id);
        }

        // Load event-seen data into memory
        GLOBALS.events.load_event_seen_data().await?;

        // Start a reaper that collects new_viewed_events and saves them to the database
        std::mem::drop(tokio::spawn(async move {
            loop {
                // sleep 5 seconds
                tokio::time::sleep(std::time::Duration::new(5, 0)).await;

                // Take all viewed events
                let ids = {
                    let map = GLOBALS.new_viewed_events.write().await;
                    let ids: Vec<Id> = map.iter().map(|elem| *elem.key()).collect();
                    map.clear();
                    ids
                };

                // Save all viewed events
                if let Err(e) = DbEventFlags::mark_all_as_viewed(ids).await {
                    tracing::error!("Could not save viewed events to database: {}", e);
                }
            }
        }));

        // Load relay lists from the database and process
        {
            let events: Vec<Event> = DbEvent::fetch_relay_lists().await?;

            // Process these events
            let mut count = 0;
            for event in events.iter() {
                count += 1;
                crate::process::process_new_event(event, false, None, None).await?;
            }
            tracing::info!("Loaded {} relay list events from the database", count);
        }

        // Pick Relays and start Minions
        if !GLOBALS.settings.read().offline {
            self.pick_relays().await;
        }

        // Separately subscribe to RelayList discovery for everyone we follow
        let discover_relay_urls: Vec<RelayUrl> =
            GLOBALS.relays_url_filtered(|r| r.has_usage_bits(DbRelay::DISCOVER));
        let followed = GLOBALS.people.get_followed_pubkeys();
        for relay_url in discover_relay_urls.iter() {
            self.engage_minion(
                relay_url.to_owned(),
                vec![RelayJob {
                    reason: "discovery",
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeDiscover(followed.clone()),
                    },
                    persistent: true,
                }],
            )
            .await?;
        }

        // Separately subscribe to our config on our write relays
        let write_relay_urls: Vec<RelayUrl> =
            GLOBALS.relays_url_filtered(|r| r.has_usage_bits(DbRelay::WRITE));
        for relay_url in write_relay_urls.iter() {
            self.engage_minion(
                relay_url.to_owned(),
                vec![RelayJob {
                    reason: "config",
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeConfig,
                    },
                    persistent: true,
                }],
            )
            .await?;
        }

        // Separately subscribe to our mentions on our read relays
        // NOTE: we also do this on all dynamically connected relays since NIP-65 is
        //       not in widespread usage.
        let read_relay_urls: Vec<RelayUrl> =
            GLOBALS.relays_url_filtered(|r| r.has_usage_bits(DbRelay::READ));
        for relay_url in read_relay_urls.iter() {
            self.engage_minion(
                relay_url.to_owned(),
                vec![RelayJob {
                    reason: "mentions",
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeMentions,
                    },
                    persistent: true,
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
                    tracing::info!("Done picking relays: {}", failure);
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
                            GLOBALS.relay_picker.relay_disconnected(&relay_url);
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
                    reason: "follow",
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeGeneralFeed(
                            assignment.pubkeys.clone(),
                        ),
                    },
                    persistent: false,
                },
                RelayJob {
                    // Until NIP-65 is in widespread use, we should listen for mentions
                    // of us on all these relays too
                    reason: "mentions",
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::SubscribeMentions,
                    },
                    persistent: false,
                },
            ],
        )
        .await?;

        Ok(())
    }

    async fn engage_minion(&mut self, url: RelayUrl, mut jobs: Vec<RelayJob>) -> Result<(), Error> {
        // Do not connect if we are offline
        if GLOBALS.settings.read().offline {
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
        task_nextjoined: Option<Result<(task::Id, ()), task::JoinError>>,
    ) {
        if task_nextjoined.is_none() {
            return; // rare but possible
        }
        match task_nextjoined.unwrap() {
            Err(join_error) => {
                let id = join_error.id();
                let maybe_url = self.minions_task_url.get(&id).cloned();
                match maybe_url {
                    Some(url) => {
                        // JoinError also has is_cancelled, is_panic, into_panic, try_into_panic
                        // Minion probably alreaedy logged, this may be redundant.
                        tracing::error!("Minion {} completed with error: {}", &url, join_error);

                        // Minion probably already logged failure in relay table

                        // Set to not connected
                        let relayjobs = GLOBALS.connected_relays.remove(&url).map(|(_, v)| v);

                        // Remove from our hashmap
                        self.minions_task_url.remove(&id);

                        // We might need to act upon this minion exiting
                        if !GLOBALS.shutting_down.load(Ordering::Relaxed) {
                            self.recover_from_minion_exit(url, relayjobs).await;
                        }
                    }
                    None => {
                        tracing::error!("Minion UNKNOWN completed with error: {}", join_error);
                    }
                }
            }
            Ok((id, _)) => {
                let maybe_url = self.minions_task_url.get(&id).cloned();
                match maybe_url {
                    Some(url) => {
                        tracing::info!("Relay Task {} completed", &url);

                        // Set to not connected
                        let relayjobs = GLOBALS.connected_relays.remove(&url).map(|(_, v)| v);

                        // Remove from our hashmap
                        self.minions_task_url.remove(&id);

                        // We might need to act upon this minion exiting
                        if !GLOBALS.shutting_down.load(Ordering::Relaxed) {
                            self.recover_from_minion_exit(url, relayjobs).await;
                        }
                    }
                    None => tracing::error!("Relay Task UNKNOWN completed"),
                }
            }
        }
    }

    async fn recover_from_minion_exit(&mut self, url: RelayUrl, jobs: Option<Vec<RelayJob>>) {
        // For people we are following, pick relays
        GLOBALS.relay_picker.relay_disconnected(&url);
        if let Err(e) = GLOBALS.relay_picker.refresh_person_relay_scores().await {
            tracing::error!("Error: {}", e);
        }
        self.pick_relays().await;

        if let Some(mut jobs) = jobs {
            // If we have any persistent jobs, restart after a delaythe relay
            let persistent_jobs: Vec<RelayJob> =
                jobs.drain(..).filter(|job| job.persistent).collect();

            if !persistent_jobs.is_empty() {
                // Do it after a delay
                std::mem::drop(tokio::spawn(async move {
                    // Delay 30 seconds first
                    tracing::info!(
                        "Minion {} will restart in 30 seconds to continue persistent jobs",
                        &url
                    );
                    tokio::time::sleep(std::time::Duration::new(30, 0)).await;
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::ReengageMinion(url, persistent_jobs));
                }));
            }
        }
    }

    async fn handle_message(&mut self, message: ToOverlordMessage) -> Result<bool, Error> {
        match message {
            ToOverlordMessage::AddRelay(relay_str) => {
                let dbrelay = DbRelay::new(relay_str.clone());
                DbRelay::insert(dbrelay.clone()).await?;
                GLOBALS.all_relays.insert(relay_str, dbrelay);
            }
            ToOverlordMessage::AdjustRelayUsageBit(relay_url, bit, value) => {
                if let Some(mut dbrelay) = GLOBALS.all_relays.get_mut(&relay_url) {
                    dbrelay.adjust_usage_bit_memory_only(bit, value);
                    dbrelay.save_usage_bits().await?;
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
                *GLOBALS.status_message.write().await = "Identity deleted.".to_string()
            }
            ToOverlordMessage::DeletePub => {
                GLOBALS.signer.clear_public_key();
                Self::delegation_reset().await?;
                GLOBALS.signer.save_through_settings().await?;
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
                // We presume the caller already checked GLOBALS.events.get() and it was not there
                for url in relay_urls.iter() {
                    self.engage_minion(
                        url.to_owned(),
                        vec![RelayJob {
                            reason: "fetch-event",
                            payload: ToMinionPayload {
                                job_id: rand::random::<u64>(),
                                detail: ToMinionPayloadDetail::FetchEvent(id.into()),
                            },
                            persistent: false,
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
                match Profile::try_from_bech32_string(&nprofile) {
                    Ok(np) => self.follow_nprofile(np).await?,
                    Err(e) => *GLOBALS.status_message.write().await = format!("{}", e),
                }
            }
            ToOverlordMessage::GeneratePrivateKey(mut password) => {
                GLOBALS.signer.generate_private_key(&password)?;
                password.zeroize();
                GLOBALS.signer.save_through_settings().await?;
            }
            ToOverlordMessage::HideOrShowRelay(relay_url, hidden) => {
                if let Some(mut relay) = GLOBALS.all_relays.get_mut(&relay_url) {
                    relay.value_mut().hidden = hidden;
                }
                DbRelay::update_hidden(relay_url, hidden).await?;
            }
            ToOverlordMessage::ImportPriv(mut import_priv, mut password) => {
                if import_priv.starts_with("ncryptsec") {
                    let epk = EncryptedPrivateKey(import_priv);
                    GLOBALS.signer.set_encrypted_private_key(epk);
                    GLOBALS.signer.unlock_encrypted_private_key(&password)?;
                    password.zeroize();
                    GLOBALS.signer.save_through_settings().await?;
                } else {
                    let maybe_pk1 = PrivateKey::try_from_bech32_string(&import_priv);
                    let maybe_pk2 = PrivateKey::try_from_hex_string(&import_priv);
                    import_priv.zeroize();
                    if maybe_pk1.is_err() && maybe_pk2.is_err() {
                        password.zeroize();
                        *GLOBALS.status_message.write().await =
                            "Private key not recognized.".to_owned();
                    } else {
                        let privkey = maybe_pk1.unwrap_or_else(|_| maybe_pk2.unwrap());
                        GLOBALS.signer.set_private_key(privkey, &password)?;
                        password.zeroize();
                        GLOBALS.signer.save_through_settings().await?;
                    }
                }
            }
            ToOverlordMessage::ImportPub(pubstr) => {
                let maybe_pk1 = PublicKey::try_from_bech32_string(&pubstr);
                let maybe_pk2 = PublicKey::try_from_hex_string(&pubstr);
                if maybe_pk1.is_err() && maybe_pk2.is_err() {
                    *GLOBALS.status_message.write().await = "Public key not recognized.".to_owned();
                } else {
                    let pubkey = maybe_pk1.unwrap_or_else(|_| maybe_pk2.unwrap());
                    GLOBALS.signer.set_public_key(pubkey);
                    GLOBALS.signer.save_through_settings().await?;
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
                            .retain(|job| job.payload.job_id != job_id || job.persistent)
                    }
                }
            }
            ToOverlordMessage::PickRelays => {
                // When manually doing this, we refresh person_relay scores first which
                // often change if the user just added follows.
                GLOBALS.relay_picker.refresh_person_relay_scores().await?;

                // Then pick
                self.pick_relays().await;
            }
            ToOverlordMessage::ProcessIncomingEvents => {
                std::mem::drop(tokio::spawn(async move {
                    for (event, url, sub) in GLOBALS.incoming_events.write().await.drain(..) {
                        let _ =
                            crate::process::process_new_event(&event, true, Some(url), sub).await;
                    }
                }));
            }
            ToOverlordMessage::PruneDatabase => {
                std::mem::drop(tokio::spawn(async move {
                    if let Err(e) = crate::db::prune().await {
                        tracing::error!("{}", e);
                    }
                }));
            }
            ToOverlordMessage::Post(content, tags, reply_to) => {
                self.post(content, tags, reply_to).await?;
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
                if let Some(mut dbrelay) = GLOBALS.all_relays.get_mut(&relay_url) {
                    dbrelay.rank = rank as u64;
                }
                DbRelay::set_rank(relay_url, rank).await?;
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
            ToOverlordMessage::SaveSettings => {
                let settings = GLOBALS.settings.read().clone();
                settings.save().await?;
                tracing::debug!("Settings saved.");
            }
            ToOverlordMessage::SetActivePerson(pubkey) => {
                GLOBALS.people.set_active_person(pubkey).await?;
            }
            ToOverlordMessage::SetThreadFeed(id, referenced_by, relays) => {
                self.set_thread_feed(id, referenced_by, relays).await?;
            }
            ToOverlordMessage::Shutdown => {
                tracing::info!("Overlord shutting down");
                return Ok(false);
            }
            ToOverlordMessage::UnlockKey(mut password) => {
                if let Err(e) = GLOBALS.signer.unlock_encrypted_private_key(&password) {
                    tracing::error!("{}", e);
                    *GLOBALS.status_message.write().await =
                        "Could not decrypt key with that password.".to_owned();
                };
                password.zeroize();

                // Update public key from private key
                let public_key = GLOBALS.signer.public_key().unwrap();
                GLOBALS.settings.write().public_key = Some(public_key);
                let settings = GLOBALS.settings.read().clone();
                settings.save().await?;
            }
            ToOverlordMessage::UpdateFollowing(merge) => {
                self.update_following(merge).await?;
            }
            ToOverlordMessage::UpdateMetadata(pubkey) => {
                let best_relays =
                    DbPersonRelay::get_best_relays(pubkey.clone(), Direction::Write).await?;
                let num_relays_per_person = GLOBALS.settings.read().num_relays_per_person;

                // we do 1 more than num_relays_per_person, which is really for main posts,
                // since metadata is more important and I didn't want to bother with
                // another setting.
                for (relay_url, _score) in
                    best_relays.iter().take(num_relays_per_person as usize + 1)
                {
                    self.engage_minion(
                        relay_url.to_owned(),
                        vec![RelayJob {
                            reason: "tmp-metadata",
                            payload: ToMinionPayload {
                                job_id: rand::random::<u64>(),
                                detail: ToMinionPayloadDetail::TempSubscribeMetadata(vec![
                                    pubkey.clone()
                                ]),
                            },
                            persistent: false,
                        }],
                    )
                    .await?;
                }

                // Mark in globals that we want to recheck their nip-05 when that metadata
                // comes in
                GLOBALS.people.recheck_nip05_on_update_metadata(&pubkey);
            }
            ToOverlordMessage::UpdateMetadataInBulk(mut pubkeys) => {
                let num_relays_per_person = GLOBALS.settings.read().num_relays_per_person;
                let mut map: HashMap<RelayUrl, Vec<PublicKeyHex>> = HashMap::new();
                for pubkey in pubkeys.drain(..) {
                    let best_relays =
                        DbPersonRelay::get_best_relays(pubkey.clone(), Direction::Write).await?;
                    for (relay_url, _score) in
                        best_relays.iter().take(num_relays_per_person as usize + 1)
                    {
                        map.entry(relay_url.to_owned())
                            .and_modify(|entry| entry.push(pubkey.clone()))
                            .or_insert_with(|| vec![pubkey.clone()]);
                    }
                }
                for (relay_url, pubkeys) in map.drain() {
                    self.engage_minion(
                        relay_url.clone(),
                        vec![RelayJob {
                            reason: "tmp-metadata",
                            payload: ToMinionPayload {
                                job_id: rand::random::<u64>(),
                                detail: ToMinionPayloadDetail::TempSubscribeMetadata(pubkeys),
                            },
                            persistent: false,
                        }],
                    )
                    .await?;
                }
            }
        }

        Ok(true)
    }

    async fn follow_pubkey_and_relay(
        &mut self,
        pubkeystr: String,
        relay: RelayUrl,
    ) -> Result<(), Error> {
        let pk = match PublicKey::try_from_bech32_string(&pubkeystr) {
            Ok(pk) => pk,
            Err(_) => PublicKey::try_from_hex_string(&pubkeystr)?,
        };
        let pkhex: PublicKeyHex = pk.into();
        GLOBALS.people.async_follow(&pkhex, true).await?;

        tracing::debug!("Followed {}", &pkhex);

        // Save relay
        let db_relay = DbRelay::new(relay.clone());
        DbRelay::insert(db_relay).await?;

        let now = Unixtime::now().unwrap().0 as u64;

        // Save person_relay
        DbPersonRelay::insert(DbPersonRelay {
            person: pkhex.to_string(),
            relay,
            last_fetched: None,
            last_suggested_kind3: Some(now), // consider it our claim in our contact list
            last_suggested_nip05: None,
            last_suggested_bytag: None,
            read: false,
            write: false,
            manually_paired_read: true,
            manually_paired_write: true,
        })
        .await?;

        // async_follow added them to the relay tracker.
        // Pick relays to start tracking them now
        self.pick_relays().await;

        tracing::info!("Setup 1 relay for {}", &pkhex);

        Ok(())
    }

    async fn post(
        &mut self,
        mut content: String,
        mut tags: Vec<Tag>,
        reply_to: Option<Id>,
    ) -> Result<(), Error> {
        // We will fill this just before we create the event
        let mut tagged_pubkeys: Vec<PublicKeyHex>;

        let event = {
            let public_key = match GLOBALS.signer.public_key() {
                Some(pk) => pk,
                None => {
                    tracing::warn!("No public key! Not posting");
                    return Ok(());
                }
            };

            if GLOBALS.settings.read().set_client_tag {
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
                    NostrBech32::Pubkey(pk) => {
                        add_pubkey_to_tags(&mut tags, pk).await;
                    }
                    NostrBech32::Profile(prof) => {
                        add_pubkey_to_tags(&mut tags, &prof.pubkey).await;
                    }
                    NostrBech32::Id(id) => {
                        // NIP-10: "Those marked with "mention" denote a quoted or reposted event id."
                        add_event_to_tags(&mut tags, *id, "mention").await;
                    }
                    NostrBech32::EventPointer(ep) => {
                        // NIP-10: "Those marked with "mention" denote a quoted or reposted event id."
                        add_event_to_tags(&mut tags, ep.id, "mention").await;
                    }
                }
            }

            // Standardize nostr links (prepend 'nostr:' where missing)
            content = NostrUrl::urlize(&content);

            if let Some(parent_id) = reply_to {
                // Get the event we are replying to
                let parent = match GLOBALS.events.get(&parent_id) {
                    Some(e) => e,
                    None => return Err("Cannot find event we are replying to.".into()),
                };

                // Add a 'p' tag for the author we are replying to (except if it is our own key)
                if parent.pubkey != public_key {
                    add_pubkey_to_tags(&mut tags, &parent.pubkey).await;
                }

                // Add all the 'p' tags from the note we are replying to (except our own)
                // FIXME: Should we avoid taging people who are muted?
                for tag in &parent.tags {
                    if let Tag::Pubkey { pubkey, .. } = tag {
                        if pubkey.as_str() != public_key.as_hex_string() {
                            add_pubkey_hex_to_tags(&mut tags, pubkey).await;
                        }
                    }
                }

                if let Some((root, _maybeurl)) = parent.replies_to_root() {
                    // Add an 'e' tag for the root
                    add_event_to_tags(&mut tags, root, "root").await;

                    // Add an 'e' tag for the note we are replying to
                    add_event_to_tags(&mut tags, parent_id, "reply").await;
                } else {
                    // We are replying to the root.
                    // NIP-10: "A direct reply to the root of a thread should have a single marked "e" tag of type "root"."
                    add_event_to_tags(&mut tags, parent_id, "root").await;
                }

                // Possibly propagate a subject tag
                for tag in &parent.tags {
                    if let Tag::Subject(subject) = tag {
                        let mut subject = subject.to_owned();
                        if !subject.starts_with("Re: ") {
                            subject = format!("Re: {}", subject);
                        }
                        subject = subject.chars().take(80).collect();
                        add_subject_to_tags_if_missing(&mut tags, subject);
                    }
                }
            }

            // Copy the tagged pubkeys for determine which relays to send to
            tagged_pubkeys = tags
                .iter()
                .filter_map(|t| {
                    if let Tag::Pubkey { pubkey, .. } = t {
                        Some(pubkey.clone())
                    } else {
                        None
                    }
                })
                .collect();

            let pre_event = PreEvent {
                pubkey: public_key,
                created_at: Unixtime::now().unwrap(),
                kind: EventKind::TextNote,
                tags,
                content,
                ots: None,
            };

            let powint = GLOBALS.settings.read().pow;
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
        crate::process::process_new_event(&event, false, None, None).await?;

        // Determine which relays to post this to
        let mut relay_urls: Vec<RelayUrl> = Vec::new();
        {
            // Get 'read' relays for everybody tagged in the event.
            // Currently we take the 2 best read relays per person
            for pubkey in tagged_pubkeys.drain(..) {
                let best_relays: Vec<RelayUrl> =
                    DbPersonRelay::get_best_relays(pubkey, Direction::Read)
                        .await?
                        .drain(..)
                        .take(2)
                        .map(|(u, _)| u)
                        .collect();
                relay_urls.extend(best_relays);
            }

            // Get all of the relays that we write to
            let write_relay_urls: Vec<RelayUrl> =
                GLOBALS.relays_url_filtered(|r| r.has_usage_bits(DbRelay::WRITE));
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
                    reason: "posting",
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvent(Box::new(event.clone())),
                    },
                    persistent: false,
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

        let inbox_or_outbox_relays: Vec<DbRelay> = GLOBALS.relays_filtered(|r| {
            r.has_usage_bits(DbRelay::INBOX) || r.has_usage_bits(DbRelay::OUTBOX)
        });
        let mut tags: Vec<Tag> = Vec::new();
        for relay in inbox_or_outbox_relays.iter() {
            tags.push(Tag::Reference {
                url: relay.url.to_unchecked_url(),
                marker: if relay.has_usage_bits(DbRelay::INBOX)
                    && relay.has_usage_bits(DbRelay::OUTBOX)
                {
                    None
                } else if relay.has_usage_bits(DbRelay::INBOX) {
                    Some("read".to_owned()) // NIP-65 uses the term 'read' instead of 'inbox'
                } else if relay.has_usage_bits(DbRelay::OUTBOX) {
                    Some("write".to_owned()) // NIP-65 uses the term 'write' instead of 'outbox'
                } else {
                    unreachable!()
                },
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

        let advertise_to_relay_urls: Vec<RelayUrl> =
            GLOBALS.relays_url_filtered(|r| r.has_usage_bits(DbRelay::ADVERTISE));

        for relay_url in advertise_to_relay_urls {
            // Send it the event to post
            tracing::debug!("Asking {} to post", &relay_url);

            self.engage_minion(
                relay_url.to_owned(),
                vec![RelayJob {
                    reason: "advertising",
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvent(Box::new(event.clone())),
                    },
                    persistent: false,
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
                    recommended_relay_url: DbRelay::recommended_relay_for_reply(id)
                        .await?
                        .map(|rr| rr.to_unchecked_url()),
                    marker: None,
                },
                Tag::Pubkey {
                    pubkey: pubkey.into(),
                    recommended_relay_url: None,
                    petname: None,
                },
            ];

            if GLOBALS.settings.read().set_client_tag {
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

            let powint = GLOBALS.settings.read().pow;
            let pow = if powint > 0 { Some(powint) } else { None };
            let (work_sender, work_receiver) = mpsc::channel();

            std::thread::spawn(move || {
                work_logger(work_receiver, powint);
            });

            GLOBALS
                .signer
                .sign_preevent(pre_event, pow, Some(work_sender))?
        };

        let relays: Vec<DbRelay> = GLOBALS.relays_filtered(|r| r.has_usage_bits(DbRelay::WRITE));
        // FIXME - post it to relays we have seen it on.

        for relay in relays {
            // Send it the event to post
            tracing::debug!("Asking {} to post", &relay.url);

            self.engage_minion(
                relay.url.clone(),
                vec![RelayJob {
                    reason: "post-like",
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvent(Box::new(event.clone())),
                    },
                    persistent: false,
                }],
            )
            .await?;
        }

        // Process the message for ourself
        crate::process::process_new_event(&event, false, None, None).await?;

        Ok(())
    }

    async fn pull_following(&mut self) -> Result<(), Error> {
        // Pull our list from all of the relays we post to
        let relays: Vec<DbRelay> = GLOBALS.relays_filtered(|r| r.has_usage_bits(DbRelay::WRITE));

        for relay in relays {
            // Send it the event to pull our followers
            tracing::debug!("Asking {} to pull our followers", &relay.url);

            self.engage_minion(
                relay.url.clone(),
                vec![RelayJob {
                    reason: "pull-contacts",
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PullFollowing,
                    },
                    persistent: false,
                }],
            )
            .await?;
        }

        Ok(())
    }

    async fn push_following(&mut self) -> Result<(), Error> {
        let event = GLOBALS.people.generate_contact_list_event().await?;

        // Push to all of the relays we post to
        let relays: Vec<DbRelay> = GLOBALS.relays_filtered(|r| r.has_usage_bits(DbRelay::WRITE));

        for relay in relays {
            // Send it the event to pull our followers
            tracing::debug!("Pushing ContactList to {}", &relay.url);

            self.engage_minion(
                relay.url.clone(),
                vec![RelayJob {
                    reason: "pushing-contacts",
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvent(Box::new(event.clone())),
                    },
                    persistent: false,
                }],
            )
            .await?;
        }

        Ok(())
    }

    async fn clear_following(&mut self) -> Result<(), Error> {
        GLOBALS.people.async_follow_none().await?;
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
        let relays: Vec<DbRelay> = GLOBALS.relays_filtered(|r| r.has_usage_bits(DbRelay::WRITE));

        for relay in relays {
            // Send it the event to pull our followers
            tracing::debug!("Pushing Metadata to {}", &relay.url);

            self.engage_minion(
                relay.url.clone(),
                vec![RelayJob {
                    reason: "write-metadata",
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvent(Box::new(event.clone())),
                    },
                    persistent: false,
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
            pubkeys.push(pubkey.into())
        }

        let num_relays_per_person = GLOBALS.settings.read().num_relays_per_person;

        let mut map: HashMap<RelayUrl, Vec<PublicKeyHex>> = HashMap::new();

        // Sort the people into the relays we will find their metadata at
        for pubkey in &pubkeys {
            for relayscore in DbPersonRelay::get_best_relays(pubkey.to_owned(), Direction::Write)
                .await?
                .drain(..)
                .take(num_relays_per_person as usize)
            {
                map.entry(relayscore.0)
                    .and_modify(|e| e.push(pubkey.to_owned()))
                    .or_insert_with(|| vec![pubkey.to_owned()]);
            }
        }

        for (url, pubkeys) in map.drain() {
            self.engage_minion(
                url.clone(),
                vec![RelayJob {
                    reason: "tmp-metadata",
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::TempSubscribeMetadata(pubkeys),
                    },
                    persistent: false,
                }],
            )
            .await?;
        }

        Ok(())
    }

    async fn repost(&mut self, id: Id) -> Result<(), Error> {
        let reposted_event = match GLOBALS.events.get(&id) {
            Some(event) => event,
            None => {
                *GLOBALS.status_message.write().await =
                    "Cannot repost - cannot find event.".to_owned();
                return Ok(());
            }
        };

        let mut tags: Vec<Tag> = vec![
            Tag::Event {
                id,
                recommended_relay_url: match GLOBALS.events.get_seen_on(&reposted_event.id) {
                    None => DbRelay::recommended_relay_for_reply(id)
                        .await?
                        .map(|rr| rr.to_unchecked_url()),
                    Some(vec) => vec.get(0).map(|rurl| rurl.to_unchecked_url()),
                },
                marker: None,
            },
            Tag::Pubkey {
                pubkey: reposted_event.pubkey.into(),
                recommended_relay_url: None,
                petname: None,
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

            if GLOBALS.settings.read().set_client_tag {
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

            let powint = GLOBALS.settings.read().pow;
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
        crate::process::process_new_event(&event, false, None, None).await?;

        // Determine which relays to post this to
        let mut relay_urls: Vec<RelayUrl> = Vec::new();
        {
            // Get all of the relays that we write to
            let write_relay_urls: Vec<RelayUrl> =
                GLOBALS.relays_url_filtered(|r| r.has_usage_bits(DbRelay::WRITE));
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
                    reason: "reposting",
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvent(Box::new(event.clone())),
                    },
                    persistent: false,
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
        relays.extend(DbEventRelay::get_relays_for_event(referenced_by).await?);
        relays.extend(DbEventRelay::get_relays_for_event(id).await?);

        // Climb the tree as high as we can, and if there are higher events,
        // we will ask for those in the initial subscription
        let highest_parent_id =
            if let Some(hpid) = GLOBALS.events.get_highest_local_parent(&id).await? {
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
        if let Some(highest_parent) = GLOBALS.events.get_local(highest_parent_id).await? {
            // Use relays in 'e' tags
            for (id, opturl) in highest_parent.referred_events() {
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

        // Load events from local database
        // FIXME: This replicates filters that the minion also builds. We should
        //        instead build the filters, then both send them to the minion and
        //        also query them locally.
        {
            if !missing_ancestors_hex.is_empty() {
                let idhp: Vec<IdHexPrefix> = missing_ancestors_hex
                    .iter()
                    .map(|id| id.to_owned().into())
                    .collect();
                let _ = GLOBALS
                    .events
                    .get_local_events_by_filter(Filter {
                        ids: idhp,
                        ..Default::default()
                    })
                    .await?;

                let kinds = GLOBALS
                    .settings
                    .read()
                    .feed_related_event_kinds()
                    .drain(..)
                    .filter(|k| k.augments_feed_related())
                    .collect();

                let e = GLOBALS
                    .events
                    .get_local_events_by_filter(Filter {
                        e: missing_ancestors_hex.clone(),
                        kinds,
                        ..Default::default()
                    })
                    .await?;
                if !e.is_empty() {
                    tracing::debug!("Loaded {} local ancestor events", e.len());
                }
            }

            let mut kinds = GLOBALS.settings.read().feed_related_event_kinds();
            kinds.retain(|f| *f != EventKind::EncryptedDirectMessage);

            let e = GLOBALS
                .events
                .get_local_events_by_filter(Filter {
                    e: vec![id.into()],
                    kinds,
                    ..Default::default()
                })
                .await?;
            if !e.is_empty() {
                tracing::debug!("Loaded {} local reply events", e.len());
            }
        }

        // Subscribe on relays
        if relays.is_empty() {
            *GLOBALS.status_message.write().await =
                "Could not find any relays for that event".to_owned();
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
                        reason: "read-thread",
                        payload: ToMinionPayload {
                            job_id: rand::random::<u64>(),
                            detail: ToMinionPayloadDetail::SubscribeThreadFeed(
                                id.into(),
                                missing_ancestors_hex.clone(),
                            ),
                        },
                        persistent: false,
                    }],
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn follow_nprofile(&mut self, nprofile: Profile) -> Result<(), Error> {
        let pubkey = nprofile.pubkey.into();
        GLOBALS.people.async_follow(&pubkey, true).await?;

        // Set their relays
        for relay in nprofile.relays.iter() {
            if let Ok(relay_url) = RelayUrl::try_from_unchecked_url(relay) {
                // Save relay
                let db_relay = DbRelay::new(relay_url.clone());
                DbRelay::insert(db_relay.clone()).await?;
                if let Entry::Vacant(entry) = GLOBALS.all_relays.entry(relay_url.clone()) {
                    entry.insert(db_relay);
                }

                // Save person_relay
                DbPersonRelay::upsert_last_suggested_nip05(
                    pubkey.to_owned(),
                    relay_url,
                    Unixtime::now().unwrap().0 as u64,
                )
                .await?;
            }
        }

        *GLOBALS.status_message.write().await =
            format!("Followed user at {} relays", nprofile.relays.len());

        // async_follow added them to the relay tracker.
        // Pick relays to start tracking them now
        self.pick_relays().await;

        Ok(())
    }

    async fn delegation_reset() -> Result<(), Error> {
        if GLOBALS.delegation.reset() {
            // save and statusmsg
            GLOBALS.delegation.save_through_settings().await?;
            *GLOBALS.status_message.write().await = "Delegation tag removed".to_string();
        }
        Ok(())
    }

    async fn delete(&mut self, id: Id) -> Result<(), Error> {
        let tags: Vec<Tag> = vec![Tag::Event {
            id,
            recommended_relay_url: None,
            marker: None,
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
        crate::process::process_new_event(&event, false, None, None).await?;

        // Determine which relays to post this to
        let mut relay_urls: Vec<RelayUrl> = Vec::new();
        {
            // Get all of the relays that we write to
            let write_relay_urls: Vec<RelayUrl> =
                GLOBALS.relays_url_filtered(|r| r.has_usage_bits(DbRelay::WRITE));
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
                    reason: "deleting",
                    payload: ToMinionPayload {
                        job_id: rand::random::<u64>(),
                        detail: ToMinionPayloadDetail::PostEvent(Box::new(event.clone())),
                    },
                    persistent: false,
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
        let event = {
            let pubkey = match GLOBALS.signer.public_key() {
                Some(pk) => pk,
                None => return Ok(()), // we cannot do anything without an identity setup first
            };
            match DbEvent::fetch_last_contact_list(pubkey.into()).await? {
                Some(event) => event,
                None => return Ok(()), // we have no contact list to update from
            }
        };

        let mut pubkeys: Vec<PublicKeyHex> = Vec::new();

        let now = Unixtime::now().unwrap();

        // 'p' tags represent the author's contacts
        for tag in &event.tags {
            if let Tag::Pubkey {
                pubkey,
                recommended_relay_url,
                petname: _,
            } = tag
            {
                // Make sure we have that person
                GLOBALS
                    .people
                    .create_all_if_missing(&[pubkey.to_owned()])
                    .await?;

                // Save the pubkey for actual following them (outside of the loop in a batch)
                pubkeys.push(pubkey.to_owned());

                // If there is a URL
                if let Some(url) = recommended_relay_url
                    .as_ref()
                    .and_then(|rru| RelayUrl::try_from_unchecked_url(rru).ok())
                {
                    // Save relay if missing
                    let db_relay = DbRelay::new(url.clone());
                    DbRelay::insert(db_relay.clone()).await?;
                    if let Entry::Vacant(entry) = GLOBALS.all_relays.entry(url.clone()) {
                        entry.insert(db_relay);
                    }

                    // create or update person_relay last_suggested_kind3
                    DbPersonRelay::upsert_last_suggested_kind3(
                        pubkey.to_string(),
                        url,
                        now.0 as u64,
                    )
                    .await?;
                }

                // TBD: do something with the petname
            }
        }

        // Follow all those pubkeys, and unfollow everbody else if merge=false
        GLOBALS.people.follow_all(&pubkeys, merge).await?;

        // Update last_contact_list_edit
        let last_edit = if merge {
            Unixtime::now().unwrap() // now, since superior to the last event
        } else {
            event.created_at
        };
        GLOBALS
            .people
            .last_contact_list_edit
            .store(last_edit.0, Ordering::Relaxed);
        {
            let db = GLOBALS.db.lock().await;
            db.execute(
                "UPDATE local_settings SET last_contact_list_edit=?",
                (last_edit.0,),
            )?;
        }

        // Pick relays again
        {
            // Refresh person-relay scores
            GLOBALS.relay_picker.refresh_person_relay_scores().await?;

            // Then pick
            self.pick_relays().await;
        }

        Ok(())
    }
}

fn work_logger(work_receiver: mpsc::Receiver<u8>, powint: u8) {
    while let Ok(work) = work_receiver.recv() {
        if work >= powint {
            // Even if work > powint, it doesn't count since we declared our target.
            *GLOBALS.status_message.blocking_write() =
                format!("Message sent with {powint} bits of work computed.");
            break;
        } else {
            *GLOBALS.status_message.blocking_write() = format!("PoW: {work}/{powint}");
        }
    }
}
