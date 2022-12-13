
use crate::{BusMessage, Error, GLOBALS, Settings};
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

mod handle_bus;

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
                self.start_minion(best_relay.relay.url.clone(),
                                  best_relay.pubkeys.clone());

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

    fn start_minion(&mut self, url: String, pubkeys: Vec<PublicKeyHex>) {
        let moved_url = Url(url.clone());
        let abort_handle = self.minions.spawn(async move {
            let mut minion = Minion::new(moved_url, pubkeys);
            minion.handle().await
        });
        let id = abort_handle.id();
        self.minions_task_url.insert(id, Url(url));
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

                                    // Remove from our hashmap
                                    self.minions_task_url.remove(&id);
                                },
                                None => {
                                    log::warn!("Minion UNKNOWN completed with error: {}", join_error);
                                }
                            }
                        },
                        Ok((id, _)) => {
                            let maybe_url = self.minions_task_url.get(&id);
                            match maybe_url {
                                Some(url) => {
                                    log::warn!("Relay Task {} completed", &url);

                                    // Remove from our hashmap
                                    self.minions_task_url.remove(&id);
                                },
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
