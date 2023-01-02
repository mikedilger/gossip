mod minion;
mod relay_picker;

use crate::comms::BusMessage;
use crate::db::{DbEvent, DbPersonRelay, DbRelay};
use crate::error::Error;
use crate::globals::{Globals, GLOBALS};
use crate::people::People;
use minion::Minion;
use nostr_types::{
    Event, EventKind, Id, Nip05, PreEvent, PrivateKey, PublicKey, PublicKeyHex, Tag, Unixtime, Url,
};
use relay_picker::{BestRelay, RelayPicker};
use std::collections::HashMap;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::{select, task};
use tracing::{debug, error, info, warn};
use zeroize::Zeroize;

pub struct Overlord {
    to_minions: Sender<BusMessage>,
    inbox: UnboundedReceiver<BusMessage>,

    // All the minion tasks running.
    minions: task::JoinSet<()>,

    // Map from minion task::Id to Url
    minions_task_url: HashMap<task::Id, Url>,

    // Vec of urls our minions are handling
    urls_watching: Vec<Url>,
}

impl Overlord {
    pub fn new(inbox: UnboundedReceiver<BusMessage>) -> Overlord {
        let to_minions = GLOBALS.to_minions.clone();
        Overlord {
            to_minions,
            inbox,
            minions: task::JoinSet::new(),
            minions_task_url: HashMap::new(),
            urls_watching: Vec::new(),
        }
    }

    pub async fn run(&mut self) {
        if let Err(e) = self.run_inner().await {
            error!("{}", e);
        }

        info!("Overlord signalling UI to shutdown");

        GLOBALS
            .shutting_down
            .store(true, std::sync::atomic::Ordering::Relaxed);

        info!("Overlord signalling minions to shutdown");

        // Send shutdown message to all minions (and ui)
        // If this fails, it's probably because there are no more listeners
        // so just ignore it and keep shutting down.
        let _ = self.to_minions.send(BusMessage {
            target: "all".to_string(),
            kind: "shutdown".to_string(),
            json_payload: serde_json::to_string("shutdown").unwrap(),
        });

        info!("Overlord waiting for minions to all shutdown");

        // Listen on self.minions until it is empty
        while !self.minions.is_empty() {
            let task_nextjoined = self.minions.join_next_with_id().await;

            self.handle_task_nextjoined(task_nextjoined).await;
        }

        info!("Overlord confirms all minions have shutdown");
    }

    pub async fn run_inner(&mut self) -> Result<(), Error> {
        // Load Signer (we cannot unlock yet, UI will have to drive that after
        // prompting for a password)
        if let Some(epk) = GLOBALS
            .settings
            .read()
            .await
            .encrypted_private_key
            .to_owned()
        {
            GLOBALS.signer.write().await.load_encrypted_private_key(epk);
        }

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
        GLOBALS.people.write().await.load_all_followed().await?;

        // Load latest metadata per person and update their metadata
        {
            let db_events = DbEvent::fetch_latest_metadata().await?;
            for dbevent in db_events.iter() {
                let e: Event = match serde_json::from_str(&dbevent.raw) {
                    Ok(e) => e,
                    Err(_) => {
                        error!("Bad raw event: id={}, raw={}", dbevent.id, dbevent.raw);
                        continue;
                    }
                };

                // Process this metadata event to update people
                crate::process::process_new_event(&e, false, None).await?;
            }
        }

        // Load feed-related events from database and process (TextNote, EventDeletion, Reaction)
        {
            let now = Unixtime::now().unwrap();
            let feed_chunk = GLOBALS.settings.read().await.feed_chunk;
            let then = now.0 - feed_chunk as i64;
            let db_events = DbEvent::fetch(Some(&format!(
                " (kind=1 OR kind=5 OR kind=7) AND created_at > {} ORDER BY created_at ASC",
                then
            )))
            .await?;

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
                crate::process::process_new_event(event, false, None).await?;
            }
            info!("Loaded {} events from the database", count);
        }

        // Pick Relays and start Minions
        if !GLOBALS.settings.read().await.offline {
            let pubkeys: Vec<PublicKeyHex> = GLOBALS
                .people
                .read()
                .await
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
                    warn!(
                        "Safety catch: we have picked {} relays. That's enough.",
                        max_relays
                    );
                    break;
                }

                if relay_picker.is_degenerate() {
                    info!(
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
                    info!("Best relay is now degenerate.");
                    break;
                }

                // Fire off a minion to handle this relay
                self.start_minion(best_relay.relay.url.clone()).await?;

                // Tell it to follow the chosen people
                let _ = self.to_minions.send(BusMessage {
                    target: best_relay.relay.url.clone(),
                    kind: "set_followed_people".to_string(),
                    json_payload: serde_json::to_string(&best_relay.pubkeys).unwrap(),
                });

                info!(
                    "Picked relay {} covering {} people.",
                    &best_relay.relay.url,
                    best_relay.pubkeys.len()
                );

                relay_count += 1;
            }

            info!("Listening on {} relays", relay_count);

            // Get desired events from relays
            self.get_missing_events().await?;
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
                    error!("{}", e);
                }
            }
        }

        Ok(())
    }

    async fn start_minion(&mut self, url: String) -> Result<(), Error> {
        if GLOBALS.settings.read().await.offline {
            return Ok(());
        }

        let url = Url::new(&url);
        if !url.is_valid_relay_url() {
            return Err(Error::InvalidUrl(url.inner().to_owned()));
        }
        let mut minion = Minion::new(url.clone()).await?;
        let abort_handle = self.minions.spawn(async move { minion.handle().await });
        let id = abort_handle.id();
        self.minions_task_url.insert(id, url.clone());
        self.urls_watching.push(url.clone());
        Ok(())
    }

    #[allow(unused_assignments)]
    async fn loop_handler(&mut self) -> Result<bool, Error> {
        let mut keepgoing: bool = true;

        tracing::debug!("overlord looping");

        if self.minions.is_empty() {
            // Just listen on inbox
            let bus_message = self.inbox.recv().await;
            let bus_message = match bus_message {
                Some(bm) => bm,
                None => {
                    // All senders dropped, or one of them closed.
                    return Ok(false);
                }
            };
            keepgoing = self.handle_bus_message(bus_message).await?;
        } else {
            // Listen on inbox, and dying minions
            select! {
                bus_message = self.inbox.recv() => {
                    let bus_message = match bus_message {
                        Some(bm) => bm,
                        None => {
                            // All senders dropped, or one of them closed.
                            return Ok(false);
                        }
                    };
                    keepgoing = self.handle_bus_message(bus_message).await?;
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
                        warn!("Minion {} completed with error: {}", &url, join_error);

                        // Minion probably already logged failure in relay table

                        // Remove from our urls_watching vec
                        self.urls_watching.retain(|value| value != url);

                        // Remove from our hashmap
                        self.minions_task_url.remove(&id);
                    }
                    None => {
                        warn!("Minion UNKNOWN completed with error: {}", join_error);
                    }
                }
            }
            Ok((id, _)) => {
                let maybe_url = self.minions_task_url.get(&id);
                match maybe_url {
                    Some(url) => {
                        info!("Relay Task {} completed", &url);

                        // Remove from our urls_watching vec
                        self.urls_watching.retain(|value| value != url);

                        // Remove from our hashmap
                        self.minions_task_url.remove(&id);
                    }
                    None => warn!("Relay Task UNKNOWN completed"),
                }
            }
        }
    }

    async fn handle_bus_message(&mut self, bus_message: BusMessage) -> Result<bool, Error> {
        #[allow(clippy::single_match)] // because temporarily so
        match &*bus_message.target {
            "all" => match &*bus_message.kind {
                "shutdown" => {
                    info!("Overlord shutting down");
                    return Ok(false);
                }
                _ => {}
            },
            "overlord" => match &*bus_message.kind {
                "minion_is_ready" => {}
                "save_settings" => {
                    GLOBALS.settings.read().await.save().await?;
                    debug!("Settings saved.");
                }
                "get_missing_events" => {
                    self.get_missing_events().await?;
                }
                "follow_nip35" => {
                    let dns_id: String = serde_json::from_str(&bus_message.json_payload)?;
                    let _ = tokio::spawn(async move {
                        if let Err(e) = Overlord::get_and_follow_nip35(dns_id).await {
                            error!("{}", e);
                        }
                    });
                }
                "follow_bech32" => {
                    let data: (String, String) = serde_json::from_str(&bus_message.json_payload)?;
                    Overlord::follow_bech32(data.0, data.1).await?;
                }
                "follow_hexkey" => {
                    let data: (String, String) = serde_json::from_str(&bus_message.json_payload)?;
                    Overlord::follow_hexkey(data.0, data.1).await?;
                }
                "unlock_key" => {
                    let mut password: String = serde_json::from_str(&bus_message.json_payload)?;
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
                "generate_private_key" => {
                    let mut password: String = serde_json::from_str(&bus_message.json_payload)?;
                    let epk = GLOBALS
                        .signer
                        .write()
                        .await
                        .generate_private_key(&password)?;
                    password.zeroize();

                    // Export and save private key
                    let public_key = GLOBALS.signer.read().await.public_key().unwrap();
                    {
                        let mut settings = GLOBALS.settings.write().await;
                        settings.encrypted_private_key = Some(epk);
                        settings.public_key = Some(public_key);
                        settings.save().await?;
                    }
                }
                "import_bech32" => {
                    let (mut import_bech32, mut password): (String, String) =
                        serde_json::from_str(&bus_message.json_payload)?;
                    let pk = PrivateKey::try_from_bech32_string(&import_bech32)?;
                    import_bech32.zeroize();
                    let epk = pk.export_encrypted(&password)?;
                    {
                        let mut signer = GLOBALS.signer.write().await;
                        signer.load_encrypted_private_key(epk.clone());
                        signer.unlock_encrypted_private_key(&password)?;
                    }
                    password.zeroize();

                    // Save
                    let public_key = GLOBALS.signer.read().await.public_key().unwrap();
                    {
                        let mut settings = GLOBALS.settings.write().await;
                        settings.encrypted_private_key = Some(epk);
                        settings.public_key = Some(public_key);
                        settings.save().await?;
                    }
                }
                "import_hex" => {
                    let (mut import_hex, mut password): (String, String) =
                        serde_json::from_str(&bus_message.json_payload)?;
                    let pk = PrivateKey::try_from_hex_string(&import_hex)?;
                    import_hex.zeroize();
                    let epk = pk.export_encrypted(&password)?;
                    {
                        let mut signer = GLOBALS.signer.write().await;
                        signer.load_encrypted_private_key(epk.clone());
                        signer.unlock_encrypted_private_key(&password)?;
                    }
                    password.zeroize();

                    // Save
                    let public_key = GLOBALS.signer.read().await.public_key().unwrap();
                    {
                        let mut settings = GLOBALS.settings.write().await;
                        settings.encrypted_private_key = Some(epk);
                        settings.public_key = Some(public_key);
                        settings.save().await?;
                    }
                }
                "save_relays" => {
                    let dirty_relays: Vec<DbRelay> = GLOBALS
                        .relays
                        .read()
                        .await
                        .iter()
                        .filter_map(|(_, r)| if r.dirty { Some(r.to_owned()) } else { None })
                        .collect();
                    info!("Saving {} relays", dirty_relays.len());
                    for relay in dirty_relays.iter() {
                        // Just update 'post' since that's all 'dirty' indicates currently
                        DbRelay::update_post(relay.url.to_owned(), relay.post).await?;
                        if let Some(relay) =
                            GLOBALS.relays.write().await.get_mut(&Url::new(&relay.url))
                        {
                            relay.dirty = false;
                        }
                    }
                }
                "post_textnote" => {
                    let content: String = serde_json::from_str(&bus_message.json_payload)?;
                    self.post_textnote(content).await?;
                }
                "post_reply" => {
                    let (content, reply_to): (String, Id) =
                        serde_json::from_str(&bus_message.json_payload)?;
                    self.post_reply(content, reply_to).await?;
                }
                "process_incoming_events" => {
                    // Clear new events
                    GLOBALS.event_is_new.write().await.clear();

                    let _ = tokio::spawn(async move {
                        for (event, url) in GLOBALS.incoming_events.write().await.drain(..) {
                            let _ =
                                crate::process::process_new_event(&event, true, Some(url)).await;
                        }
                    });
                }
                "add_relay" => {
                    let relay_str: String = serde_json::from_str(&bus_message.json_payload)?;
                    if let Ok(dbrelay) = DbRelay::new(relay_str) {
                        DbRelay::insert(dbrelay).await?;
                    }
                }
                _ => {}
            },
            _ => {}
        }

        Ok(true)
    }

    async fn get_missing_events(&mut self) -> Result<(), Error> {
        let (desired_events_map, orphans): (HashMap<Url, Vec<Id>>, Vec<Id>) =
            Globals::get_desired_events().await?;

        let desired_count = GLOBALS.desired_events.read().await.len();

        if desired_count == 0 {
            return Ok(());
        }

        info!("Seeking {} events", desired_count);

        let urls: Vec<Url> = desired_events_map
            .keys()
            .map(|u| u.to_owned())
            .filter(|u| u.is_valid_relay_url())
            .collect();

        for url in urls.iter() {
            // Get all the ones slated for this relay
            let mut ids = desired_events_map.get(url).cloned().unwrap_or_default();

            // Add the orphans
            ids.extend(&orphans);

            if ids.is_empty() {
                continue;
            }

            // If we don't have such a minion, start one
            if !self.urls_watching.contains(url) {
                // Start a minion
                self.start_minion(url.inner().to_owned()).await?;
            }

            debug!("{}: Asking to fetch {} events", url.inner(), ids.len());

            // Tell it to get these events
            let _ = self.to_minions.send(BusMessage {
                target: url.inner().to_owned(),
                kind: "fetch_events".to_string(),
                json_payload: serde_json::to_string(&ids).unwrap(),
            });
        }

        Ok(())
    }

    async fn get_and_follow_nip35(nip35: String) -> Result<(), Error> {
        let mut parts: Vec<&str> = nip35.split('@').collect();
        if parts.len() != 2 {
            return Err(Error::InvalidDnsId);
        }

        let domain = parts.pop().unwrap();
        let user = parts.pop().unwrap();
        let nip05_future = reqwest::Client::new()
            .get(format!(
                "https://{}/.well-known/nostr.json?name={}",
                domain, user
            ))
            .header("Host", domain)
            .send();
        let timeout_future = tokio::time::timeout(std::time::Duration::new(15, 0), nip05_future);
        let response = timeout_future.await??;
        let nip05 = response.json::<Nip05>().await?;
        Overlord::follow_nip35(nip05, user.to_string(), domain.to_string()).await?;
        Ok(())
    }

    async fn follow_nip35(nip05: Nip05, user: String, domain: String) -> Result<(), Error> {
        let dns_id = format!("{}@{}", user, domain);

        let pubkey = match nip05.names.get(&user) {
            Some(pk) => pk,
            None => return Err(Error::Nip05NotFound),
        };

        // Save person
        GLOBALS
            .people
            .write()
            .await
            .upsert_valid_nip05(
                &(*pubkey).into(),
                dns_id.clone(),
                Unixtime::now().unwrap().0 as u64,
            )
            .await?;

        // Mark as followed
        GLOBALS
            .people
            .write()
            .await
            .async_follow(&(*pubkey).into(), true)
            .await?;

        info!("Followed {}", &dns_id);

        let relays = match nip05.relays.get(pubkey) {
            Some(relays) => relays,
            None => return Err(Error::Nip35NotFound),
        };

        for relay in relays.iter() {
            // Save relay
            let relay_url = Url::new(relay);
            if relay_url.is_valid_relay_url() {
                let db_relay = DbRelay::new(relay_url.inner().to_owned())?;
                DbRelay::insert(db_relay).await?;

                // Save person_relay
                DbPersonRelay::upsert_last_suggested_nip35(
                    (*pubkey).into(),
                    relay.inner().to_owned(),
                    Unixtime::now().unwrap().0 as u64,
                )
                .await?;
            }
        }

        info!("Setup {} relays for {}", relays.len(), &dns_id);

        Ok(())
    }

    async fn follow_bech32(bech32: String, relay: String) -> Result<(), Error> {
        let pk = PublicKey::try_from_bech32_string(&bech32)?;
        let pkhex: PublicKeyHex = pk.into();
        GLOBALS
            .people
            .write()
            .await
            .async_follow(&pkhex, true)
            .await?;

        debug!("Followed {}", &pkhex);

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

        info!("Setup 1 relay for {}", &pkhex);

        Ok(())
    }

    async fn follow_hexkey(hexkey: String, relay: String) -> Result<(), Error> {
        let pk = PublicKey::try_from_hex_string(&hexkey)?;
        let pkhex: PublicKeyHex = pk.into();
        GLOBALS
            .people
            .write()
            .await
            .async_follow(&pkhex, true)
            .await?;

        debug!("Followed {}", &pkhex);

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

        info!("Setup 1 relay for {}", &pkhex);

        Ok(())
    }

    async fn post_textnote(&mut self, content: String) -> Result<(), Error> {
        let event = {
            let public_key = match GLOBALS.signer.read().await.public_key() {
                Some(pk) => pk,
                None => {
                    warn!("No public key! Not posting");
                    return Ok(());
                }
            };

            let pre_event = PreEvent {
                pubkey: public_key,
                created_at: Unixtime::now().unwrap(),
                kind: EventKind::TextNote,
                tags: vec![],
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
            if !self.urls_watching.contains(&Url::new(&relay.url)) {
                self.start_minion(relay.url.clone()).await?;
            }

            // Send it the event to post
            debug!("Asking {} to post", &relay.url);

            let _ = self.to_minions.send(BusMessage {
                target: relay.url.clone(),
                kind: "post_event".to_string(),
                json_payload: serde_json::to_string(&event).unwrap(),
            });
        }

        // Process the message for ourself
        crate::process::process_new_event(&event, false, None).await?;

        Ok(())
    }

    async fn post_reply(&mut self, content: String, reply_to: Id) -> Result<(), Error> {
        let event = {
            let public_key = match GLOBALS.signer.read().await.public_key() {
                Some(pk) => pk,
                None => {
                    warn!("No public key! Not posting");
                    return Ok(());
                }
            };

            let pre_event = PreEvent {
                pubkey: public_key,
                created_at: Unixtime::now().unwrap(),
                kind: EventKind::TextNote,
                tags: vec![Tag::Event {
                    id: reply_to,
                    recommended_relay_url: DbRelay::recommended_relay_for_reply(reply_to).await?,
                    marker: Some("reply".to_string()),
                }],
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
            if !self.urls_watching.contains(&Url::new(&relay.url)) {
                self.start_minion(relay.url.clone()).await?;
            }

            // Send it the event to post
            debug!("Asking {} to post", &relay.url);

            let _ = self.to_minions.send(BusMessage {
                target: relay.url.clone(),
                kind: "post_event".to_string(),
                json_payload: serde_json::to_string(&event).unwrap(),
            });
        }

        // Process the message for ourself
        crate::process::process_new_event(&event, false, None).await?;

        Ok(())
    }
}
