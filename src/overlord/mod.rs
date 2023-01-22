mod minion;
mod relay_picker;

use crate::comms::{ToMinionMessage, ToMinionPayload, ToOverlordMessage};
use crate::db::{DbEvent, DbEventSeen, DbPersonRelay, DbRelay};
use crate::error::Error;
use crate::globals::GLOBALS;
use crate::people::People;
use crate::tags::{
    add_event_to_tags, add_pubkey_hex_to_tags, add_pubkey_to_tags, keys_from_text, notes_from_text,
};
use minion::Minion;
use nostr_types::{
    EncryptedPrivateKey, Event, EventKind, Id, IdHex, Metadata, PreEvent, PrivateKey, Profile,
    PublicKey, PublicKeyHex, Tag, Unixtime, Url,
};
use relay_picker::{BestRelay, RelayPicker};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
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
    minions_task_url: HashMap<task::Id, Url>,
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
            payload: ToMinionPayload::Shutdown,
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
        GLOBALS.signer.write().await.load_from_settings().await;

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
        let all_relays = DbRelay::fetch(None).await?;

        // Store copy of all relays in globals (we use it again down below)
        for relay in all_relays.iter() {
            GLOBALS
                .relays
                .write()
                .await
                .insert(Url::new(&relay.url), relay.clone());
        }

        // Load people from the database
        GLOBALS.people.load_all_followed().await?;

        // Load latest metadata per person and update their metadata
        // This can happen in the background
        task::spawn(async move {
            if let Ok(db_events) = DbEvent::fetch_latest_metadata().await {
                for dbevent in db_events.iter() {
                    let e: Event = match serde_json::from_str(&dbevent.raw) {
                        Ok(e) => e,
                        Err(_) => {
                            tracing::error!(
                                "Bad raw event: id={}, raw={}",
                                dbevent.id,
                                dbevent.raw
                            );
                            continue;
                        }
                    };

                    // Process this metadata event to update people
                    if let Err(e) = crate::process::process_new_event(&e, false, None, None).await {
                        tracing::error!("{}", e);
                    }
                }
            }
        });

        let now = Unixtime::now().unwrap();

        // Load reply-related events from database and process
        // (where you are tagged)
        {
            let replies_chunk = GLOBALS.settings.read().await.replies_chunk;
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
            let feed_chunk = GLOBALS.settings.read().await.feed_chunk;
            let then = now.0 - feed_chunk as i64;

            let cond = if GLOBALS.settings.read().await.reactions {
                format!(" (kind=1 OR kind=5 OR kind=6 OR kind=7) AND created_at > {} ORDER BY created_at ASC", then)
            } else {
                format!(
                    " (kind=1 OR kind=5 OR kind=6) AND created_at > {} ORDER BY created_at ASC",
                    then
                )
            };
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
        }

        // Pick Relays and start Minions
        if !GLOBALS.settings.read().await.offline {
            let pubkeys: Vec<PublicKeyHex> = GLOBALS
                .people
                .get_followed_pubkeys()
                .iter()
                .map(|p| p.to_owned())
                .collect();

            let (num_relays_per_person, max_relays) = {
                let settings = GLOBALS.settings.read().await;
                (settings.num_relays_per_person, settings.max_relays)
            };
            let mut pubkey_counts: HashMap<PublicKeyHex, u8> = HashMap::new();
            for pk in pubkeys.iter() {
                pubkey_counts.insert(pk.clone(), num_relays_per_person);
            }

            let mut relay_picker = RelayPicker {
                relays: all_relays,
                pubkey_counts,
                person_relays: DbPersonRelay::fetch_for_pubkeys(&pubkeys).await?,
            };

            let mut best_relay: BestRelay;
            let mut relay_count = 0;
            loop {
                if relay_count >= max_relays {
                    tracing::info!(
                        "Safety catch: we have picked {} relays. That's enough.",
                        max_relays
                    );
                    break;
                }

                if relay_picker.is_degenerate() {
                    tracing::debug!(
                        "Relay picker is degenerate, relays={} pubkey_counts={}, person_relays={}",
                        relay_picker.relays.len(),
                        relay_picker.pubkey_counts.len(),
                        relay_picker.person_relays.len()
                    );
                    break;
                }

                let (rd, rp) = relay_picker.best()?;
                best_relay = rd;
                relay_picker = rp;

                if best_relay.is_degenerate() {
                    tracing::debug!("Best relay is now degenerate.");
                    break;
                }

                // Fire off a minion to handle this relay
                self.start_minion(best_relay.relay.url.clone()).await?;

                // Subscribe to the general feed
                // FIXME: older code sent in &best_relay.pubkeys, but minions
                // stopped doing anything with that.
                let _ = self.to_minions.send(ToMinionMessage {
                    target: best_relay.relay.url.clone(),
                    payload: ToMinionPayload::SubscribeGeneralFeed,
                });

                tracing::info!(
                    "Picked relay {} covering {} people.",
                    &best_relay.relay.url,
                    best_relay.pubkeys.len()
                );

                relay_count += 1;
            }

            tracing::info!("Listening on {} relays", relay_count);
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

    async fn start_minion(&mut self, url: String) -> Result<(), Error> {
        let (offline, max_relays) = {
            let settings = GLOBALS.settings.read().await;
            (settings.offline, settings.max_relays)
        };

        if offline {
            return Ok(());
        }

        if GLOBALS.relays_watching.read().await.len() >= max_relays.into() {
            return Err(Error::MaxRelaysReached);
        }

        let url = Url::new(&url);
        if !url.is_valid_relay_url() {
            return Err(Error::InvalidUrl(url.inner().to_owned()));
        }
        let mut minion = Minion::new(url.clone()).await?;
        let abort_handle = self.minions.spawn(async move { minion.handle().await });
        let id = abort_handle.id();
        self.minions_task_url.insert(id, url.clone());
        GLOBALS.relays_watching.write().await.push(url.clone());
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
                let maybe_url = self.minions_task_url.get(&id);
                match maybe_url {
                    Some(url) => {
                        // JoinError also has is_cancelled, is_panic, into_panic, try_into_panic
                        // Minion probably alreaedy logged, this may be redundant.
                        tracing::error!("Minion {} completed with error: {}", &url, join_error);

                        // Minion probably already logged failure in relay table

                        // Remove from our urls_watching vec
                        GLOBALS
                            .relays_watching
                            .write()
                            .await
                            .retain(|value| value != url);

                        // Remove from our hashmap
                        self.minions_task_url.remove(&id);
                    }
                    None => {
                        tracing::error!("Minion UNKNOWN completed with error: {}", join_error);
                    }
                }
            }
            Ok((id, _)) => {
                let maybe_url = self.minions_task_url.get(&id);
                match maybe_url {
                    Some(url) => {
                        tracing::info!("Relay Task {} completed", &url);

                        // Remove from our urls_watching vec
                        GLOBALS
                            .relays_watching
                            .write()
                            .await
                            .retain(|value| value != url);

                        // Remove from our hashmap
                        self.minions_task_url.remove(&id);
                    }
                    None => tracing::error!("Relay Task UNKNOWN completed"),
                }
            }
        }
    }

    async fn handle_message(&mut self, message: ToOverlordMessage) -> Result<bool, Error> {
        match message {
            ToOverlordMessage::AddRelay(relay_str) => {
                let dbrelay = DbRelay::new(relay_str)?;
                DbRelay::insert(dbrelay).await?;
            }
            ToOverlordMessage::DeletePub => {
                GLOBALS.signer.write().await.clear_public_key();
                GLOBALS.signer.read().await.save_through_settings().await?;
            }
            ToOverlordMessage::FollowPubkeyAndRelay(pubkeystr, relay) => {
                Overlord::follow_pubkey_and_relay(pubkeystr, relay).await?;
            }
            ToOverlordMessage::FollowNip05(nip05) => {
                let _ = tokio::spawn(async move {
                    if let Err(e) = crate::nip05::get_and_follow_nip05(nip05).await {
                        tracing::error!("{}", e);
                    }
                });
            }
            ToOverlordMessage::FollowNprofile(nprofile) => {
                match Profile::try_from_bech32_string(&nprofile) {
                    Ok(np) => self.follow_nprofile(np).await?,
                    Err(e) => *GLOBALS.status_message.write().await = format!("{}", e),
                }
            }
            ToOverlordMessage::GeneratePrivateKey(mut password) => {
                GLOBALS
                    .signer
                    .write()
                    .await
                    .generate_private_key(&password)?;
                password.zeroize();
                GLOBALS.signer.read().await.save_through_settings().await?;
            }
            ToOverlordMessage::ImportPriv(mut import_priv, mut password) => {
                if import_priv.starts_with("ncryptsec") {
                    let epk = EncryptedPrivateKey(import_priv);
                    GLOBALS.signer.write().await.set_encrypted_private_key(epk);
                    GLOBALS
                        .signer
                        .write()
                        .await
                        .unlock_encrypted_private_key(&password)?;
                    password.zeroize();
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
                        GLOBALS
                            .signer
                            .write()
                            .await
                            .set_private_key(privkey, &password)?;
                        password.zeroize();
                        GLOBALS.signer.read().await.save_through_settings().await?;
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
                    GLOBALS.signer.write().await.set_public_key(pubkey);
                    GLOBALS.signer.read().await.save_through_settings().await?;
                }
            }
            ToOverlordMessage::Like(id, pubkey) => {
                self.post_like(id, pubkey).await?;
            }
            ToOverlordMessage::MinionIsReady => {
                // currently ignored
            }
            ToOverlordMessage::ProcessIncomingEvents => {
                // Clear new events
                GLOBALS.events.clear_new();

                let _ = tokio::spawn(async move {
                    for (event, url, sub) in GLOBALS.incoming_events.write().await.drain(..) {
                        let _ =
                            crate::process::process_new_event(&event, true, Some(url), sub).await;
                    }
                });
            }
            ToOverlordMessage::PruneDatabase => {
                let _ = tokio::spawn(async move {
                    if let Err(e) = crate::db::prune().await {
                        tracing::error!("{}", e);
                    }
                });
            }
            ToOverlordMessage::PostReply(content, tags, reply_to) => {
                self.post_reply(content, tags, reply_to).await?;
            }
            ToOverlordMessage::PostTextNote(content, tags) => {
                self.post_textnote(content, tags).await?;
            }
            ToOverlordMessage::PullFollowMerge => {
                self.pull_following(true).await?;
            }
            ToOverlordMessage::PullFollowOverwrite => {
                self.pull_following(false).await?;
            }
            ToOverlordMessage::PushFollow => {
                self.push_following().await?;
            }
            ToOverlordMessage::PushMetadata(metadata) => {
                self.push_metadata(metadata).await?;
            }
            ToOverlordMessage::RefreshFollowedMetadata => {
                self.refresh_followed_metadata().await?;
            }
            ToOverlordMessage::SaveRelays => {
                let dirty_relays: Vec<DbRelay> = GLOBALS
                    .relays
                    .read()
                    .await
                    .iter()
                    .filter_map(|(_, r)| if r.dirty { Some(r.to_owned()) } else { None })
                    .collect();
                tracing::info!("Saving {} relays", dirty_relays.len());
                for relay in dirty_relays.iter() {
                    // Just update 'post' since that's all 'dirty' indicates currently
                    DbRelay::update_post(relay.url.to_owned(), relay.post).await?;
                    if let Some(relay) = GLOBALS.relays.write().await.get_mut(&Url::new(&relay.url))
                    {
                        relay.dirty = false;
                    }
                }
            }
            ToOverlordMessage::SaveSettings => {
                GLOBALS.settings.read().await.save().await?;
                tracing::debug!("Settings saved.");
            }
            ToOverlordMessage::SetThreadFeed(id, referenced_by) => {
                self.set_thread_feed(id, referenced_by).await?;
            }
            ToOverlordMessage::Shutdown => {
                tracing::info!("Overlord shutting down");
                return Ok(false);
            }
            ToOverlordMessage::UnlockKey(mut password) => {
                GLOBALS
                    .signer
                    .write()
                    .await
                    .unlock_encrypted_private_key(&password)?;
                password.zeroize();

                // Update public key from private key
                let public_key = GLOBALS.signer.read().await.public_key().unwrap();
                {
                    let mut settings = GLOBALS.settings.write().await;
                    settings.public_key = Some(public_key);
                    settings.save().await?;
                }
            }
            ToOverlordMessage::UpdateMetadata(pubkey) => {
                let person_relays = DbPersonRelay::fetch_for_pubkeys(&[pubkey.clone()]).await?;

                for person_relay in person_relays.iter() {
                    // Start a minion for this relay if there is none
                    if !GLOBALS
                        .relays_watching
                        .read()
                        .await
                        .contains(&Url::new(&person_relay.relay))
                    {
                        self.start_minion(person_relay.relay.clone()).await?;
                    }

                    // Subscribe to metadata and contact lists for this person
                    let _ = self.to_minions.send(ToMinionMessage {
                        target: person_relay.relay.to_string(),
                        payload: ToMinionPayload::TempSubscribeMetadata(vec![pubkey.clone()]),
                    });
                }
            }
        }

        Ok(true)
    }

    async fn follow_pubkey_and_relay(pubkeystr: String, relay: String) -> Result<(), Error> {
        let pk = match PublicKey::try_from_bech32_string(&pubkeystr) {
            Ok(pk) => pk,
            Err(_) => PublicKey::try_from_hex_string(&pubkeystr)?,
        };
        let pkhex: PublicKeyHex = pk.into();
        GLOBALS.people.async_follow(&pkhex, true).await?;

        tracing::debug!("Followed {}", &pkhex);

        // Save relay
        let relay_url = Url::new(&relay);
        if !relay_url.is_valid_relay_url() {
            return Err(Error::InvalidUrl(relay));
        }
        let db_relay = DbRelay::new(relay.to_string())?;
        DbRelay::insert(db_relay).await?;

        // Save person_relay
        DbPersonRelay::insert(DbPersonRelay {
            person: pkhex.0.clone(),
            relay: relay_url.inner().to_owned(),
            ..Default::default()
        })
        .await?;

        tracing::info!("Setup 1 relay for {}", &pkhex);

        Ok(())
    }

    async fn post_textnote(
        &mut self,
        mut content: String,
        mut tags: Vec<Tag>,
    ) -> Result<(), Error> {
        let event = {
            let public_key = match GLOBALS.signer.read().await.public_key() {
                Some(pk) => pk,
                None => {
                    tracing::warn!("No public key! Not posting");
                    return Ok(());
                }
            };

            if GLOBALS.settings.read().await.set_client_tag {
                tags.push(Tag::Other {
                    tag: "client".to_owned(),
                    data: vec!["gossip".to_owned()],
                });
            }

            // Add tags for keys that are in the post body as npub1...
            for (npub, pubkey) in keys_from_text(&content) {
                let idx = add_pubkey_to_tags(&mut tags, pubkey).await;
                content = content.replace(&npub, &format!("#[{}]", idx));
            }

            // Do the same as above, but now with note1...
            for (npub, pubkey) in notes_from_text(&content) {
                let idx = add_event_to_tags(&mut tags, pubkey, "mention").await;
                content = content.replace(&npub, &format!("#[{}]", idx));
            }

            // Finally build the event
            let pre_event = PreEvent {
                pubkey: public_key,
                created_at: Unixtime::now().unwrap(),
                kind: EventKind::TextNote,
                tags,
                content,
                ots: None,
            };

            let powint = GLOBALS.settings.read().await.pow;
            let pow = if powint > 0 { Some(powint) } else { None };
            GLOBALS.signer.read().await.sign_preevent(pre_event, pow)?
        };

        let relays: Vec<DbRelay> = GLOBALS
            .relays
            .read()
            .await
            .iter()
            .filter_map(|(_, r)| if r.post { Some(r.to_owned()) } else { None })
            .collect();

        for relay in relays {
            // Start a minion for it, if there is none
            if !GLOBALS
                .relays_watching
                .read()
                .await
                .contains(&Url::new(&relay.url))
            {
                self.start_minion(relay.url.clone()).await?;
            }

            // Send it the event to post
            tracing::debug!("Asking {} to post", &relay.url);

            let _ = self.to_minions.send(ToMinionMessage {
                target: relay.url.clone(),
                payload: ToMinionPayload::PostEvent(Box::new(event.clone())),
            });
        }

        // Process the message for ourself
        crate::process::process_new_event(&event, false, None, None).await?;

        Ok(())
    }

    async fn post_reply(
        &mut self,
        mut content: String,
        mut tags: Vec<Tag>,
        reply_to: Id,
    ) -> Result<(), Error> {
        let event = {
            let public_key = match GLOBALS.signer.read().await.public_key() {
                Some(pk) => pk,
                None => {
                    tracing::warn!("No public key! Not posting");
                    return Ok(());
                }
            };

            // Get the event we are replying to
            let event = match GLOBALS.events.get(&reply_to) {
                Some(e) => e,
                None => {
                    return Err(Error::General(
                        "Cannot find event we are replying to.".to_owned(),
                    ))
                }
            };

            // Add a 'p' tag for the author we are replying to (except if it is our own key)
            if event.pubkey != public_key {
                add_pubkey_to_tags(&mut tags, event.pubkey).await;
            }

            // Add all the 'p' tags from the note we are replying to (except our own)
            for tag in &event.tags {
                if let Tag::Pubkey { pubkey, .. } = tag {
                    if pubkey.0 != public_key.as_hex_string() {
                        add_pubkey_hex_to_tags(&mut tags, pubkey).await;
                    }
                }
            }

            // Add tags for keys that are in the post body as npub1...
            for (npub, pubkey) in keys_from_text(&content) {
                let idx = add_pubkey_to_tags(&mut tags, pubkey).await;
                content = content.replace(&npub, &format!("#[{}]", idx));
            }

            // Do the same as above, but now with note1...
            for (npub, pubkey) in notes_from_text(&content) {
                // NIP-10: "Those marked with "mention" denote a quoted or reposted event id."
                let idx = add_event_to_tags(&mut tags, pubkey, "mention").await;
                content = content.replace(&npub, &format!("#[{}]", idx));
            }

            if let Some((root, _maybeurl)) = event.replies_to_root() {
                // Add an 'e' tag for the root
                add_event_to_tags(&mut tags, root, "root").await;

                // Add an 'e' tag for the note we are replying to
                add_event_to_tags(&mut tags, reply_to, "reply").await;
            } else {
                // We are replying to the root.
                // NIP-10: "A direct reply to the root of a thread should have a single marked "e" tag of type "root"."
                add_event_to_tags(&mut tags, reply_to, "root").await;
            }

            if GLOBALS.settings.read().await.set_client_tag {
                tags.push(Tag::Other {
                    tag: "client".to_owned(),
                    data: vec!["gossip".to_owned()],
                });
            }

            let pre_event = PreEvent {
                pubkey: public_key,
                created_at: Unixtime::now().unwrap(),
                kind: EventKind::TextNote,
                tags,
                content,
                ots: None,
            };

            let powint = GLOBALS.settings.read().await.pow;
            let pow = if powint > 0 { Some(powint) } else { None };
            GLOBALS.signer.read().await.sign_preevent(pre_event, pow)?
        };

        let relays: Vec<DbRelay> = GLOBALS
            .relays
            .read()
            .await
            .iter()
            .filter_map(|(_, r)| if r.post { Some(r.to_owned()) } else { None })
            .collect();

        for relay in relays {
            // Start a minion for it, if there is none
            if !GLOBALS
                .relays_watching
                .read()
                .await
                .contains(&Url::new(&relay.url))
            {
                self.start_minion(relay.url.clone()).await?;
            }

            // Send it the event to post
            tracing::debug!("Asking {} to post", &relay.url);

            let _ = self.to_minions.send(ToMinionMessage {
                target: relay.url.clone(),
                payload: ToMinionPayload::PostEvent(Box::new(event.clone())),
            });
        }

        // Process the message for ourself
        crate::process::process_new_event(&event, false, None, None).await?;

        Ok(())
    }

    async fn post_like(&mut self, id: Id, pubkey: PublicKey) -> Result<(), Error> {
        let event = {
            let public_key = match GLOBALS.signer.read().await.public_key() {
                Some(pk) => pk,
                None => {
                    tracing::warn!("No public key! Not posting");
                    return Ok(());
                }
            };

            let mut tags: Vec<Tag> = vec![
                Tag::Event {
                    id,
                    recommended_relay_url: DbRelay::recommended_relay_for_reply(id).await?,
                    marker: None,
                },
                Tag::Pubkey {
                    pubkey: pubkey.into(),
                    recommended_relay_url: None,
                    petname: None,
                },
            ];

            if GLOBALS.settings.read().await.set_client_tag {
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

            let powint = GLOBALS.settings.read().await.pow;
            let pow = if powint > 0 { Some(powint) } else { None };
            GLOBALS.signer.read().await.sign_preevent(pre_event, pow)?
        };

        let relays: Vec<DbRelay> = GLOBALS
            .relays
            .read()
            .await
            .iter()
            .filter_map(|(_, r)| if r.post { Some(r.to_owned()) } else { None })
            .collect();

        for relay in relays {
            // Start a minion for it, if there is none
            if !GLOBALS
                .relays_watching
                .read()
                .await
                .contains(&Url::new(&relay.url))
            {
                self.start_minion(relay.url.clone()).await?;
            }

            // Send it the event to post
            tracing::debug!("Asking {} to post", &relay.url);

            let _ = self.to_minions.send(ToMinionMessage {
                target: relay.url.clone(),
                payload: ToMinionPayload::PostEvent(Box::new(event.clone())),
            });
        }

        // Process the message for ourself
        crate::process::process_new_event(&event, false, None, None).await?;

        Ok(())
    }

    async fn pull_following(&mut self, merge: bool) -> Result<(), Error> {
        // Set globally whether we are merging or not when newer following lists
        // come in.
        GLOBALS.pull_following_merge.store(merge, Ordering::Relaxed);

        // Pull our list from all of the relays we post to
        let relays: Vec<DbRelay> = GLOBALS
            .relays
            .read()
            .await
            .iter()
            .filter_map(|(_, r)| if r.post { Some(r.to_owned()) } else { None })
            .collect();

        for relay in relays {
            // Start a minion for it, if there is none
            if !GLOBALS
                .relays_watching
                .read()
                .await
                .contains(&Url::new(&relay.url))
            {
                self.start_minion(relay.url.clone()).await?;
            }

            // Send it the event to pull our followers
            tracing::debug!("Asking {} to pull our followers", &relay.url);

            let _ = self.to_minions.send(ToMinionMessage {
                target: relay.url.clone(),
                payload: ToMinionPayload::PullFollowing,
            });
        }

        // When the event comes in, process will handle it with our global
        // merge preference.

        Ok(())
    }

    async fn push_following(&mut self) -> Result<(), Error> {
        let event = GLOBALS.people.generate_contact_list_event().await?;

        // Push to all of the relays we post to
        let relays: Vec<DbRelay> = GLOBALS
            .relays
            .read()
            .await
            .iter()
            .filter_map(|(_, r)| if r.post { Some(r.to_owned()) } else { None })
            .collect();

        for relay in relays {
            // Start a minion for it, if there is none
            if !GLOBALS
                .relays_watching
                .read()
                .await
                .contains(&Url::new(&relay.url))
            {
                self.start_minion(relay.url.clone()).await?;
            }

            // Send it the event to pull our followers
            tracing::debug!("Pushing ContactList to {}", &relay.url);

            let _ = self.to_minions.send(ToMinionMessage {
                target: relay.url.clone(),
                payload: ToMinionPayload::PostEvent(Box::new(event.clone())),
            });
        }

        Ok(())
    }

    async fn push_metadata(&mut self, metadata: Metadata) -> Result<(), Error> {
        let public_key = match GLOBALS.signer.read().await.public_key() {
            Some(pk) => pk,
            None => return Err(Error::NoPrivateKey), // not even a public key
        };

        let pre_event = PreEvent {
            pubkey: public_key,
            created_at: Unixtime::now().unwrap(),
            kind: EventKind::Metadata,
            tags: vec![],
            content: serde_json::to_string(&metadata)?,
            ots: None,
        };

        let event = GLOBALS.signer.read().await.sign_preevent(pre_event, None)?;

        // Push to all of the relays we post to
        let relays: Vec<DbRelay> = GLOBALS
            .relays
            .read()
            .await
            .iter()
            .filter_map(|(_, r)| if r.post { Some(r.to_owned()) } else { None })
            .collect();

        for relay in relays {
            // Start a minion for it, if there is none
            if !GLOBALS
                .relays_watching
                .read()
                .await
                .contains(&Url::new(&relay.url))
            {
                self.start_minion(relay.url.clone()).await?;
            }

            // Send it the event to pull our followers
            tracing::debug!("Pushing Metadata to {}", &relay.url);

            let _ = self.to_minions.send(ToMinionMessage {
                target: relay.url.clone(),
                payload: ToMinionPayload::PostEvent(Box::new(event.clone())),
            });
        }

        Ok(())
    }

    // This gets it whether we had it or not. Because it might have changed.
    async fn refresh_followed_metadata(&mut self) -> Result<(), Error> {
        let pubkeys = GLOBALS.people.get_followed_pubkeys();

        let num_relays_per_person = GLOBALS.settings.read().await.num_relays_per_person;

        let mut map: HashMap<Url, Vec<PublicKeyHex>> = HashMap::new();

        // Sort the people into the relays we will find their metadata at
        for pubkey in &pubkeys {
            for relay in DbPersonRelay::get_best_relays(pubkey.to_owned())
                .await?
                .drain(..)
                .take(num_relays_per_person as usize)
            {
                map.entry(relay)
                    .and_modify(|e| e.push(pubkey.to_owned()))
                    .or_insert_with(|| vec![pubkey.to_owned()]);
            }
        }

        for (url, pubkeys) in map.drain() {
            // Start minion if needed
            if !GLOBALS.relays_watching.read().await.contains(&url) {
                self.start_minion(url.inner().to_string()).await?;
            }

            // Subscribe to their metadata
            let _ = self.to_minions.send(ToMinionMessage {
                target: url.inner().to_string(),
                payload: ToMinionPayload::TempSubscribeMetadata(pubkeys),
            });
        }

        Ok(())
    }

    async fn set_thread_feed(&mut self, id: Id, referenced_by: Id) -> Result<(), Error> {
        // Cancel current thread subscriptions, if any
        let _ = self.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload::UnsubscribeThreadFeed,
        });

        // Collect missing ancestors and relays they might be at.
        // We will ask all the relays about all the ancestors, which is more than we need to
        // but isn't too much to ask for.
        let mut missing_ancestors: Vec<Id> = Vec::new();
        let mut relays: Vec<Url> = Vec::new();

        // Include the relays where the referenced_by event was seen
        relays.extend(DbEventSeen::get_relays_for_event(referenced_by).await?);
        relays.extend(DbEventSeen::get_relays_for_event(id).await?);
        if relays.is_empty() {
            panic!("Our method STILL isn't good enough. We need fallback read relays. I hope this panic does not occur.");
        }

        // Climb the tree as high as we can, and if there are higher events,
        // we will ask for those in the initial subscription
        if let Some(highest_parent_id) = GLOBALS.events.get_highest_local_parent(&id).await? {
            GLOBALS.feed.set_thread_parent(highest_parent_id);
            if let Some(highest_parent) = GLOBALS.events.get_local(highest_parent_id).await? {
                for (id, opturl) in highest_parent.replies_to_ancestors() {
                    missing_ancestors.push(id);
                    if let Some(url) = opturl {
                        relays.push(url);
                    }
                }
            }
        } else {
            GLOBALS.feed.set_thread_parent(id);
            missing_ancestors.push(id);
        }

        let missing_ancestors_hex: Vec<IdHex> =
            missing_ancestors.iter().map(|id| (*id).into()).collect();
        tracing::debug!("Seeking ancestors {:?}", missing_ancestors_hex);

        // Clean up relays
        relays = relays
            .drain(..)
            .filter(|u| u.is_valid_relay_url())
            .collect();
        relays.sort();
        relays.dedup();

        for url in relays.iter() {
            // Start minion if needed
            if !GLOBALS.relays_watching.read().await.contains(url) {
                self.start_minion(url.inner().to_string()).await?;
            }

            // Subscribe
            let _ = self.to_minions.send(ToMinionMessage {
                target: url.inner().to_string(),
                payload: ToMinionPayload::SubscribeThreadFeed(
                    id.into(),
                    missing_ancestors_hex.clone(),
                ),
            });
        }

        Ok(())
    }

    async fn follow_nprofile(&mut self, nprofile: Profile) -> Result<(), Error> {
        let pubkey = nprofile.pubkey.into();
        GLOBALS.people.async_follow(&pubkey, true).await?;

        // Set their relays
        for relay in nprofile.relays.iter() {
            // Save relay
            let relay_url = Url::new(relay);
            if relay_url.is_valid_relay_url() {
                let db_relay = DbRelay::new(relay_url.inner().to_owned())?;
                DbRelay::insert(db_relay.clone()).await?;

                if let Entry::Vacant(entry) = GLOBALS.relays.write().await.entry(relay_url.clone())
                {
                    entry.insert(db_relay);
                }

                // Save person_relay
                DbPersonRelay::upsert_last_suggested_nip05(
                    pubkey.to_owned(),
                    relay.inner().to_owned(),
                    Unixtime::now().unwrap().0 as u64,
                )
                .await?;
            }
        }

        *GLOBALS.status_message.write().await =
            format!("Followed user at {} relays", nprofile.relays.len());

        Ok(())
    }
}
