mod minion;
mod relay_picker;

use crate::comms::BusMessage;
use crate::db::{DbEvent, DbPerson, DbPersonRelay, DbRelay, DbSetting};
use crate::error::Error;
use crate::globals::GLOBALS;
use crate::settings::Settings;
use minion::Minion;
use nostr_types::{Event, EventKind, Metadata, PrivateKey, PublicKey, PublicKeyHex, Unixtime, Url};
use relay_picker::{BestRelay, RelayPicker};
use std::collections::HashMap;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::{select, task};
use tracing::{debug, error, info};

pub struct Overlord {
    settings: Settings,
    to_minions: Sender<BusMessage>,
    from_minions: UnboundedReceiver<BusMessage>,
    minions: task::JoinSet<()>,
    minions_task_url: HashMap<task::Id, Url>,
    #[allow(dead_code)]
    private_key: Option<PrivateKey>, // note that PrivateKey already zeroizes on drop
}

impl Overlord {
    pub fn new(from_minions: UnboundedReceiver<BusMessage>) -> Overlord {
        let to_minions = GLOBALS.to_minions.clone();
        Overlord {
            settings: Settings::default(),
            to_minions,
            from_minions,
            minions: task::JoinSet::new(),
            minions_task_url: HashMap::new(),
            private_key: None,
        }
    }

    pub async fn run(&mut self) {
        if let Err(e) = self.run_inner().await {
            error!("{}", e);
        }

        // Send shutdown message to all minions (and ui)
        // If this fails, it's probably because there are no more listeners
        // so just ignore it and keep shutting down.
        let _ = self.to_minions.send(BusMessage {
            target: "all".to_string(),
            kind: "shutdown".to_string(),
            json_payload: serde_json::to_string("shutdown").unwrap(),
        });

        // Wait on all minions to finish. When there are no more senders
        // sending to `from_minions` then they are all completed.
        // In that case this call will return an error, but we don't care we
        // just finish.
        let _ = self.from_minions.recv();
    }

    pub async fn run_inner(&mut self) -> Result<(), Error> {
        // Check for a private key
        if DbSetting::fetch_setting("user_private_key")
            .await?
            .is_some()
        {
            // We don't bother loading the value just yet because we don't have
            // the password.
            info!("Saved private key found. Will need a password to unlock.");
            GLOBALS
                .need_password
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }

        // FIXME - if this needs doing, it should be done dynamically as
        //         new people are encountered, not batch-style on startup.
        // Create a person record for every person seen, possibly autofollow
        DbPerson::populate_new_people(self.settings.autofollow).await?;

        // FIXME - if this needs doing, it should be done dynamically as
        //         new people are encountered, not batch-style on startup.
        // Create a relay record for every relay in person_relay map (these get
        // updated from events without necessarily updating our relays list)
        DbRelay::populate_new_relays().await?;

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
                let metadata: Metadata = match serde_json::from_str(&dbevent.content) {
                    Ok(e) => e,
                    Err(_) => {
                        error!(
                            "Bad metadata: id={}, content={}",
                            dbevent.id, dbevent.content
                        );
                        continue;
                    }
                };

                // Update in globals
                crate::globals::update_person_from_event_metadata(
                    e.pubkey,
                    e.created_at,
                    metadata.clone(),
                )
                .await;

                // Update in database
                DbPerson::update_metadata(
                    PublicKeyHex(e.pubkey.as_hex_string()),
                    metadata,
                    e.created_at,
                )
                .await?;
            }
        }

        // Load feed-related events from database and process (TextNote, EventDeletion, Reaction)
        {
            let now = Unixtime::now().unwrap();
            let then = now.0 - self.settings.feed_chunk as i64;
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
                crate::globals::add_event(event).await?;
            }
            info!("Loaded {} events from the database", count);
        }

        // Pick Relays and start Minions
        {
            let pubkeys: Vec<PublicKeyHex> = crate::globals::followed_pubkeys().await;

            let mut relay_picker = RelayPicker {
                relays: DbRelay::fetch(None).await?,
                pubkeys: pubkeys.clone(),
                person_relays: DbPersonRelay::fetch_for_pubkeys(&pubkeys).await?,
            };
            let mut best_relay: BestRelay;
            loop {
                if relay_picker.is_degenerate() {
                    break;
                }

                let (rd, rp) = relay_picker.best()?;
                best_relay = rd;
                relay_picker = rp;

                if best_relay.is_degenerate() {
                    break;
                }

                // Fire off a minion to handle this relay
                self.start_minion(best_relay.relay.url.clone(), best_relay.pubkeys.clone())
                    .await?;

                info!(
                    "Picked relay {}, {} people left",
                    best_relay.relay.url,
                    relay_picker.pubkeys.len()
                );
            }
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

    async fn start_minion(&mut self, url: String, pubkeys: Vec<PublicKeyHex>) -> Result<(), Error> {
        let moved_url = Url(url.clone());
        let mut minion = Minion::new(moved_url, pubkeys).await?;
        let abort_handle = self.minions.spawn(async move { minion.handle().await });
        let id = abort_handle.id();
        self.minions_task_url.insert(id, Url(url));

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
        }

        Ok(keepgoing)
    }

    async fn handle_bus_message(&mut self, bus_message: BusMessage) -> Result<bool, Error> {
        #[allow(clippy::single_match)] // because temporarily so
        match &*bus_message.target {
            "all" => match &*bus_message.kind {
                "shutdown" => {
                    info!("Overlord shutting down");
                    return Ok(false);
                }
                "settings_changed" => {
                    self.settings = serde_json::from_str(&bus_message.json_payload)?;
                    // We need to inform the minions
                    self.to_minions.send(BusMessage {
                        target: "all".to_string(),
                        kind: "settings_changed".to_string(),
                        json_payload: bus_message.json_payload.clone(),
                    })?;
                }
                _ => {}
            },
            "overlord" => match &*bus_message.kind {
                "new_event" => {
                    let event: Event = serde_json::from_str(&bus_message.json_payload)?;

                    // If feed-related, send to the feed event processor
                    if event.kind == EventKind::TextNote
                        || event.kind == EventKind::EncryptedDirectMessage
                        || event.kind == EventKind::EventDeletion
                        || event.kind == EventKind::Reaction
                    {
                        crate::globals::add_event(&event).await?;

                        debug!("new feed event arrived: {}...", event.id.as_hex_string());
                    } else {
                        // Not Feed Related:  Metadata, RecommendRelay, ContactList
                        debug!(
                            "new non-feed event arrived: {}...",
                            event.id.as_hex_string()
                        );

                        if event.kind == EventKind::Metadata {
                            let metadata: Metadata = serde_json::from_str(&event.content)?;
                            crate::globals::update_person_from_event_metadata(
                                event.pubkey,
                                event.created_at,
                                metadata,
                            )
                            .await;
                        }

                        // FIXME: Handle EventKind::RecommendedRelay
                        // FIXME: Handle EventKind::ContactList
                    }
                }
                "minion_is_ready" => {},
                "save_settings" => {
                    let settings: Settings = serde_json::from_str(&bus_message.json_payload)?;

                    // Save to database
                    settings.save().await?; // to database

                    // Update in globals
                    *GLOBALS.settings.lock().await = settings;

                    debug!("Settings saved.");
                },
                _ => {}
            },
            _ => {}
        }

        Ok(true)
    }
}
