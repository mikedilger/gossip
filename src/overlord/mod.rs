mod minion;
mod relay_picker;

use crate::comms::BusMessage;
use crate::db::{DbEvent, DbPerson, DbPersonRelay, DbRelay};
use crate::error::Error;
use crate::globals::{Globals, GLOBALS};
use crate::settings::Settings;
use minion::Minion;
use nostr_types::{Event, Nip05, PrivateKey, PublicKey, PublicKeyHex, Unixtime, Url};
use relay_picker::{BestRelay, RelayPicker};
use std::collections::HashMap;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::{select, task};
use tracing::{debug, error, info, warn};

pub struct Overlord {
    to_minions: Sender<BusMessage>,
    from_minions: UnboundedReceiver<BusMessage>,

    // All the minion tasks running.
    minions: task::JoinSet<()>,

    // Map from minion task::Id to Url
    minions_task_url: HashMap<task::Id, Url>,

    // Vec of urls our minions are handling
    urls_watching: Vec<Url>,

    #[allow(dead_code)]
    private_key: Option<PrivateKey>, // note that PrivateKey already zeroizes on drop
}

impl Overlord {
    pub fn new(from_minions: UnboundedReceiver<BusMessage>) -> Overlord {
        let to_minions = GLOBALS.to_minions.clone();
        Overlord {
            to_minions,
            from_minions,
            minions: task::JoinSet::new(),
            minions_task_url: HashMap::new(),
            urls_watching: Vec::new(),
            private_key: None,
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
        // FIXME - if this needs doing, it should be done dynamically as
        //         new people are encountered, not batch-style on startup.
        // Create a person record for every person seen, possibly autofollow

        let autofollow = GLOBALS.settings.lock().await.autofollow;
        DbPerson::populate_new_people(autofollow).await?;

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
                .lock()
                .await
                .insert(Url(relay.url.clone()), relay.clone());
        }

        // Load people from the database
        {
            let mut dbpeople = DbPerson::fetch(None).await?;
            for dbperson in dbpeople.drain(..) {
                let pubkey = PublicKey::try_from(dbperson.pubkey.clone())?;
                GLOBALS.people.lock().await.insert(pubkey, dbperson);
            }
        }

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
            let feed_chunk = GLOBALS.settings.lock().await.feed_chunk;
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
        {
            let pubkeys: Vec<PublicKeyHex> = crate::globals::followed_pubkeys().await;
            let num_relays_per_person = GLOBALS.settings.lock().await.num_relays_per_person;
            let max_relays = GLOBALS.settings.lock().await.max_relays;
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
        }

        // Get desired events from relays
        self.get_missing_events().await?;

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
        let moved_url = Url(url.clone());
        let mut minion = Minion::new(moved_url).await?;
        let abort_handle = self.minions.spawn(async move { minion.handle().await });
        let id = abort_handle.id();
        self.minions_task_url.insert(id, Url(url.clone()));
        self.urls_watching.push(Url(url.clone()));
        Ok(())
    }

    #[allow(unused_assignments)]
    async fn loop_handler(&mut self) -> Result<bool, Error> {
        let mut keepgoing: bool = true;

        select! {
            bus_message = self.from_minions.recv() => {
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
                    // from ui
                    let settings: Settings = serde_json::from_str(&bus_message.json_payload)?;

                    // Save to database
                    settings.save().await?; // to database

                    // Update in globals
                    *GLOBALS.settings.lock().await = settings;

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
                _ => {}
            },
            _ => {}
        }

        Ok(true)
    }

    async fn get_missing_events(&mut self) -> Result<(), Error> {
        let (desired_events_map, desired_events_vec) = Globals::get_desired_events().await?;

        let desired_count = GLOBALS.desired_events.lock().await.len();

        if desired_count == 0 {
            return Ok(());
        }

        info!("Seeking {} events", desired_count);

        let urls = self.urls_watching.clone();

        for url in urls.iter() {
            // Get all the ones slated for this relay
            let mut ids = desired_events_map.get(url).cloned().unwrap_or_default();

            // Add the orphans
            ids.extend(&desired_events_vec);

            if ids.is_empty() {
                continue;
            }

            // If we don't have such a minion, start one
            if !self.urls_watching.contains(url) {
                // Start a minion
                self.start_minion(url.0.clone()).await?;
            }

            debug!("{}: Asking to fetch {} events", &url.0, ids.len());

            // Tell it to get these events
            let _ = self.to_minions.send(BusMessage {
                target: url.0.clone(),
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
        DbPerson::upsert_valid_nip05(
            (*pubkey).into(),
            dns_id.clone(),
            Unixtime::now().unwrap().0 as u64,
        )
        .await?;

        info!("Followed {}", &dns_id);

        let relays = match nip05.relays.get(pubkey) {
            Some(relays) => relays,
            None => return Err(Error::Nip35NotFound),
        };

        for relay in relays.iter() {
            // Save relay
            let relay_url = Url::new_validated(relay)?;
            let db_relay = DbRelay::new(relay_url.0)?;
            DbRelay::insert(db_relay).await?;

            // Save person_relay
            DbPersonRelay::upsert_last_suggested_nip35(
                (*pubkey).into(),
                relay.0.clone(),
                Unixtime::now().unwrap().0 as u64,
            )
            .await?;
        }

        info!("Setup {} relays for {}", relays.len(), &dns_id);

        Ok(())
    }

    async fn follow_bech32(bech32: String, relay: String) -> Result<(), Error> {
        let pk = PublicKey::try_from_bech32_string(&bech32)?;
        let pkhex: PublicKeyHex = pk.into();
        DbPerson::follow(pkhex.clone()).await?;

        debug!("Followed {}", &pkhex);

        // Save relay
        let relay_url = Url::new_validated(&relay)?;
        let db_relay = DbRelay::new(relay.to_string())?;
        DbRelay::insert(db_relay).await?;

        // Save person_relay
        DbPersonRelay::insert(DbPersonRelay {
            person: pkhex.0.clone(),
            relay: relay_url.0.clone(),
            ..Default::default()
        })
        .await?;

        info!("Setup 1 relay for {}", &pkhex);

        Ok(())
    }

    async fn follow_hexkey(hexkey: String, relay: String) -> Result<(), Error> {
        let pk = PublicKey::try_from_hex_string(&hexkey)?;
        let pkhex: PublicKeyHex = pk.into();
        DbPerson::follow(pkhex.clone()).await?;

        debug!("Followed {}", &pkhex);

        // Save relay
        let relay_url = Url::new_validated(&relay)?;
        let db_relay = DbRelay::new(relay.to_string())?;
        DbRelay::insert(db_relay).await?;

        // Save person_relay
        DbPersonRelay::insert(DbPersonRelay {
            person: pkhex.0.clone(),
            relay: relay_url.0.clone(),
            ..Default::default()
        })
        .await?;

        info!("Setup 1 relay for {}", &pkhex);

        Ok(())
    }
}
