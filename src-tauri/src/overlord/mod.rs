
use crate::{BusMessage, Error, GLOBALS, PasswordPacket, Settings};
use crate::db::{DbEvent, DbPerson, DbPersonRelay, DbRelay, DbSetting};
use nostr_proto::{Event, Metadata, PrivateKey, PublicKeyHex, Unixtime, Url};
use rusqlite::Connection;
use std::collections::HashMap;
use std::fs;
use tauri::{AppHandle, Manager};
use tokio::{select, task};
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::UnboundedReceiver;

mod event_processor;
use event_processor::EventProcessor;

mod minion;
use minion::Minion;

mod relay_picker;
use relay_picker::{BestRelay, RelayPicker};

mod js_event;
use js_event::JsEvent;

pub struct Overlord {
    app_handle: AppHandle,
    javascript_is_ready: bool,
    early_messages_to_javascript: Vec<BusMessage>,
    settings: Settings,
    to_minions: Sender<BusMessage>,
    from_minions: UnboundedReceiver<BusMessage>,
    minions: task::JoinSet<()>,
    minions_task_url: HashMap<task::Id, Url>,
    event_processor: EventProcessor,
    private_key: Option<PrivateKey>, // note that PrivateKey already zeroizes on drop
}

impl Overlord {
    pub fn new(app_handle: AppHandle, from_minions: UnboundedReceiver<BusMessage>)
               -> Overlord
    {
        let to_minions = GLOBALS.to_minions.clone();
        Overlord {
            app_handle,
            javascript_is_ready: false,
            early_messages_to_javascript: Vec::new(),
            settings: Default::default(),
            to_minions, from_minions,
            minions: task::JoinSet::new(),
            minions_task_url: HashMap::new(),
            event_processor: EventProcessor::new(),
            private_key: None,
        }
    }

    fn send_to_javascript(&mut self, bus_message: BusMessage) -> Result<(), Error> {
        if self.javascript_is_ready {
            log::trace!(
                "sending to javascript: kind={} payload={}",
                bus_message.kind,
                bus_message.payload
            );
            self.app_handle.emit_all("from_rust", bus_message)?;
        } else {
            log::debug!("PUSHING early message");
            self.early_messages_to_javascript.push(bus_message);
        }
        Ok(())
    }

    pub async fn run(&mut self) {
        if let Err(e) = self.run_inner().await {
            log::error!("{}", e);
            if let Err(e) = self.to_minions.send(BusMessage {
                relay_url: None,
                target: "all".to_string(),
                kind: "shutdown".to_string(),
                payload: "shutdown".to_string(),
            }) {
                log::error!("Unable to send shutdown: {}", e);
            }
            self.app_handle.exit(1);
            return;
        }
    }

    pub async fn run_inner(&mut self) -> Result<(), Error> {

        // Setup the database (possibly create, possibly upgrade)
        setup_database().await?;

        // Load settings
        self.settings.load().await?;

        // Tell javascript our setings
        self.send_to_javascript(BusMessage {
            relay_url: None,
            target: "javascript".to_string(),
            kind: "setsettings".to_string(),
            payload: serde_json::to_string(&self.settings)?,
        })?;


        // Load our private key
        if let Some(_) = DbSetting::fetch_setting("user_private_key").await? {
            // We don't bother loading the value just yet because we don't have
            // the password.

            // Tell javascript we need the password
            self.send_to_javascript(BusMessage {
                relay_url: None,
                target: "javascript".to_string(),
                kind: "needpassword".to_string(),
                payload: serde_json::to_string("")?,
            })?;
        } else if let Some(upkn) = DbSetting::fetch_setting("user_private_key_naked").await? {
            self.private_key = Some(PrivateKey::try_from_hex_string(&upkn)?);

            // Let javascript know our public key
            self.send_to_javascript(BusMessage {
                relay_url: None,
                target: "javascript".to_string(),
                kind: "publickey".to_string(),
                payload: serde_json::to_string(
                    &self.private_key.as_ref().unwrap().public_key().as_hex_string()
                )?
            })?;
        }

        // Create a person record for every person seen, possibly autofollow
        DbPerson::populate_new_people(self.settings.autofollow!=0).await?;

        // Create a relay record for every relay in person_relay map (these get
        // updated from events without necessarily updating our relays list)
        DbRelay::populate_new_relays().await?;

        // Send all relays to javascript
        {
            let relays = DbRelay::fetch(None).await?;

            self.send_to_javascript(BusMessage {
                relay_url: None,
                target: "javascript".to_string(),
                kind: "setrelays".to_string(),
                payload: serde_json::to_string(&relays)?,
            })?;
        }

        // Load feed-related events from database and process (TextNote, EventDeletion, Reaction)
        {
            let now = Unixtime::now().unwrap();
            let then = now.0 - self.settings.feed_chunk as i64;
            let db_events = DbEvent::fetch(Some(
                &format!(" (kind=1 OR kind=5 OR kind=7) AND created_at > {} ORDER BY created_at ASC", then)
            )).await?;

            // Map db events into Events
            let mut events: Vec<Event> = Vec::with_capacity(db_events.len());
            for dbevent in db_events.iter() {
                let e = serde_json::from_str(&dbevent.raw)?;
                events.push(e);
            }

            // Process these events
            self.event_processor.add_events(&*events);

            // Send processed JsEvents to javascript
            self.send_to_javascript(BusMessage {
                relay_url: None,
                target: "javascript".to_string(),
                kind: "setevents".to_string(),
                payload: serde_json::to_string(&self.event_processor.get_js_events())?,
            })?;

            // Send computed feed to javascript
            self.send_to_javascript(BusMessage {
                relay_url: None,
                target: "javascript".to_string(),
                kind: "replacefeed".to_string(),
                payload: serde_json::to_string(&self.event_processor.get_feed())?,
            })?;
        }

        // Update DbPerson records from kind=0 metadata events
        {
            // Get the latest kind=0 metadata update events from the database
            let mut map: HashMap<PublicKeyHex, DbEvent> = HashMap::new();
            let mut metadata_events = DbEvent::fetch(Some("kind=0")).await?;
            for me in metadata_events.drain(..) {
                let x = map.entry(me.pubkey.clone()).or_insert(me.clone());
                if x.created_at < me.created_at {
                    *x = me
                }
            }

            // Update the person records for these, and save the people
            for (_,event) in map.iter() {
                let metadata: Metadata = serde_json::from_str(&event.content)?;
                let person = DbPerson::fetch_one(event.pubkey.clone()).await?;
                if let Some(mut person) = person {
                    person.name = Some(metadata.name);
                    person.about = metadata.about;
                    person.picture = metadata.picture;
                    person.dns_id = metadata.nip05;
                    DbPerson::update(person).await?;
                }
            }
        }

        // Load all people we are following
        let people = DbPerson::fetch(Some("followed=1")).await?;

        // Send these people to javascript
        self.send_to_javascript(BusMessage {
            relay_url: None,
            target: "javascript".to_string(),
            kind: "setpeople".to_string(),
            payload: serde_json::to_string(&people)?
        })?;

        // Pick Relays and start Minions
        {
            let pubkeys: Vec<PublicKeyHex> = people.iter().map(|p| p.pubkey.clone()).collect();

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
                {
                    let url = Url(best_relay.relay.url.clone());
                    let pubkeys = best_relay.pubkeys.clone();
                    let abort_handle = self.minions.spawn(async move {
                        let mut minion = Minion::new(url, pubkeys);
                        minion.handle().await
                    });
                    let id = abort_handle.id();

                    self.minions_task_url.insert(id, Url(best_relay.relay.url.clone()));
                }

                log::info!("Picked relay {}, {} people left",
                           best_relay.relay.url,
                           relay_picker.pubkeys.len());
            }
        }

        'mainloop:
        loop {
            match self.loop_handler().await {
                Ok(keepgoing) => {
                    if !keepgoing {
                        break 'mainloop;
                    }
                },
                Err(e) => {
                    // Log them and keep looping
                    log::error!("{}", e);
                }
            }
        }

        self.app_handle.exit(1);

        // TODO:
        // Figure out what relays we need to talk to
        // Start threads for each of them
        // Refigure it out and tell them

        Ok(())
    }

    async fn loop_handler(&mut self) -> Result<bool, Error> {
        let mut keepgoing: bool = true;

        if self.minions.is_empty() {
            // We only need to listen on the bus
            let bus_message = match self.from_minions.recv().await {
                Some(bm) => bm,
                None => {
                    // All senders dropped, or one of them closed.
                    return Ok(false);
                }
            };
            keepgoing = self.handle_bus_message(bus_message).await?;
        } else {
            // We need to listen on the bus, and for completed tasks
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
                task_next_joined = self.minions.join_next_with_id() => {
                    if task_next_joined.is_none() {
                        return Ok(true); // rare
                    }
                    match task_next_joined.unwrap() {
                        Err(join_error) => {
                            let id = join_error.id();
                            let maybe_url = self.minions_task_url.get(&id);
                            match maybe_url {
                                Some(url) => {
                                    // JoinError also has is_cancelled, is_panic, into_panic, try_into_panic
                                    log::warn!("Minion {} completed with error: {}", &url, join_error);
                                },
                                None => {
                                    log::warn!("Minion UNKNOWN completed with error: {}", join_error);
                                }
                            }
                        },
                        Ok((id, _)) => {
                            let maybe_url = self.minions_task_url.get(&id);
                            match maybe_url {
                                Some(url) => log::warn!("Relay Task {} completed", &url),
                                None => log::warn!("Relay Task UNKNOWN completed"),
                            }
                        }
                    }
                    // FIXME: we should look up which relay it was serving
                    // Then we should wait for a cooldown period.
                    // Then we should recompute the filters and spin up a new task to
                    // continue that relay.
                }
            }
        }

        Ok(keepgoing)
    }

    async fn handle_bus_message(&mut self, bus_message: BusMessage) -> Result<bool, Error> {
        match &*bus_message.target {
            "javascript" => {
                self.send_to_javascript(bus_message)?;
            }
            "all" => match &*bus_message.kind {
                "shutdown" => {
                    log::info!("Overlord shutting down");
                    return Ok(false);
                },
                "settings_changed" => {
                    self.settings = serde_json::from_str(&bus_message.payload)?;
                    // We need to inform the minions
                    self.to_minions.send(BusMessage {
                        relay_url: None,
                        target: "all".to_string(),
                        kind: "settings_changed".to_string(),
                        payload: bus_message.payload.clone(),
                    })?;
                },
                _ => {}
            },
            "overlord" => match &*bus_message.kind {
                "javascript_is_ready" => {
                    log::info!("Javascript is ready");
                    self.javascript_is_ready = true;
                    self.send_early_messages_to_javascript()?;
                },
                "minion_is_ready" => {
                    // We don't bother with this. We don't send minions messages
                    // early on. In the future when we spin up new minions
                    // after startup we may need this.
                },
                "new_event" => {
                    // FIXME - on startup, relays will stream a lot of events
                    //         quickly. Rather than making all these fast changes
                    //         on the frontend, maybe we should batch them up
                    //         and update the front end every 500ms or so?

                    let event: Event = serde_json::from_str(&bus_message.payload)?;
                    let changed_events = self.event_processor.add_events(&[event]);

                    log::info!("Received new event from {} affecting {} events",
                               bus_message.relay_url.as_ref().unwrap(),
                               changed_events.len());

                    // Update javascript with added and changed events
                    self.send_to_javascript(BusMessage {
                        relay_url: None,
                        target: "javascript".to_string(),
                        kind: "setevents".to_string(),
                        payload: serde_json::to_string(&changed_events)?,
                    })?;

                    // Update javascript, replace feed
                    self.send_to_javascript(BusMessage {
                        relay_url: None,
                        target: "javascript".to_string(),
                        kind: "replacefeed".to_string(),
                        payload: serde_json::to_string(&self.event_processor.get_feed())?,
                    })?;
                },
                "generate" => {
                    let password: PasswordPacket = serde_json::from_str(&bus_message.payload)?;

                    self.private_key = Some(PrivateKey::generate());

                    let encrypted_private_key = self.private_key.as_ref().unwrap()
                        .export_encrypted(&password.0)?;

                    let user_private_key_setting = DbSetting {
                        key: "user_private_key".to_string(),
                        value: encrypted_private_key.clone()
                    };

                    DbSetting::set(user_private_key_setting).await?;

                    // Let javascript know our public key
                    self.send_to_javascript(BusMessage {
                        relay_url: None,
                        target: "javascript".to_string(),
                        kind: "publickey".to_string(),
                        payload: serde_json::to_string(
                            &self.private_key.as_ref().unwrap().public_key().as_hex_string()
                        )?
                    })?;
                },
                "unlock" => {
                    let password: PasswordPacket = serde_json::from_str(&bus_message.payload)?;

                    if let Some(epk) = DbSetting::fetch_setting("user_private_key").await? {
                        self.private_key = Some(PrivateKey::import_encrypted(&epk, &password.0)?);

                        // Let javascript know our public key
                        self.send_to_javascript(BusMessage {
                            relay_url: None,
                            target: "javascript".to_string(),
                            kind: "publickey".to_string(),
                            payload: serde_json::to_string(
                                &self.private_key.as_ref().unwrap().public_key().as_hex_string()
                            )?
                        })?;
                    }
                },
                "import_key" => {
                    let private_key_hex = serde_json::from_str(&bus_message.payload)?;
                    self.private_key = Some(PrivateKey::try_from_hex_string(private_key_hex)?);

                    // Save it
                    let user_private_key_setting = DbSetting {
                        key: "user_private_key_naked".to_string(),
                        value: private_key_hex.to_string(),
                    };
                    DbSetting::set(user_private_key_setting).await?;

                    // Let javascript know our public key
                    self.send_to_javascript(BusMessage {
                        relay_url: None,
                        target: "javascript".to_string(),
                        kind: "publickey".to_string(),
                        payload: serde_json::to_string(
                            &self.private_key.as_ref().unwrap().public_key().as_hex_string()
                        )?
                    })?;
                }
                _ => {}
            },
            _ => {}
        }

        Ok(true)
    }

    fn send_early_messages_to_javascript(&mut self) -> Result<(), Error> {
        for bus_message in self.early_messages_to_javascript.drain(..) {
            log::debug!("POPPING early message");
            log::trace!(
                "sending to javascript: kind={} payload={}",
                bus_message.kind,
                bus_message.payload
            );
            self.app_handle.emit_all("from_rust", bus_message)?;
        }
        Ok(())
    }
}

// This sets up the database
async fn setup_database() -> Result<(), Error> {
    let mut data_dir = dirs::data_dir().ok_or::<Error>(
        "Cannot find a directory to store application data.".into(),
    )?;
    data_dir.push("gossip");

    // Create our data directory only if it doesn't exist
    fs::create_dir_all(&data_dir)?;

    // Connect to (or create) our database
    let mut db_path = data_dir.clone();
    db_path.push("gossip.sqlite");
    let connection = Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
            | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX
            | rusqlite::OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;

    // Save the connection globally
    {
        let mut db = GLOBALS.db.lock().await;
        *db = Some(connection);
    }

    // Check and upgrade our data schema
    crate::db::check_and_upgrade().await?;

    Ok(())
}
