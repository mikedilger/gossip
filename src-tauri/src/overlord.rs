
use crate::{BusMessage, Error, GLOBALS};
use crate::db::{DbEvent, DbPerson, DbRelay};
use nostr_proto::{Filters, IdHex, Metadata, PublicKeyHex, Unixtime, Url};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use tauri::{AppHandle, Manager};
use tokio::{select, task};

#[derive(Clone, Debug, Serialize)]
pub struct Reactions {
    pub upvotes: u64,
    pub downvotes: u64,
    pub emojis: Vec<(char, u64)>
}

#[derive(Clone, Debug, Serialize)]
pub struct EventMetadata {
    pub id: IdHex,
    pub replies: Vec<IdHex>,
    pub reactions: Reactions
}

pub struct Overlord {
    app_handle: AppHandle,
    javascript_is_ready: bool,
    early_messages_to_javascript: Vec<BusMessage>,
}

impl Overlord {
    pub fn new(app_handle: AppHandle) -> Overlord {
        Overlord {
            app_handle,
            javascript_is_ready: false,
            early_messages_to_javascript: Vec::new()
        }
    }

    pub async fn run(&mut self) {
        if let Err(e) = self.run_inner().await {
            let tx = GLOBALS.bus.clone();
            log::error!("{}", e);
            if let Err(e) = tx.send(BusMessage {
                target: "all".to_string(),
                source: "overlord".to_string(),
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

        // Get the broadcast channel and subscribe to it
        let tx = GLOBALS.bus.clone();
        let mut rx = tx.subscribe();

        // Setup the database (possibly create, possibly upgrade)
        setup_database().await?;

        // Load the initial relay filters
        let relay_filters = crate::nostr::load_initial_relay_filters().await?;

        // Keep the join handles for the relay tasks
        let mut relay_tasks = task::JoinSet::new();
        // Keep a mapping from Task ID to relay url
        let mut task_id_to_relay_url: HashMap<task::Id, Url> = HashMap::new();

        // Create a person record for every person seen, and follow everybody
        DbPerson::populate_new_people(true).await?;

        // Create a relay record for every relay in person_relay map (these get
        // updated from events without necessarily updating our relays list)
        DbRelay::populate_new_relays().await?;

        // Load TextNote event data from database and send to javascript
        {
            let now = Unixtime::now().unwrap();
            let then = now.0 - 43200; // 1 day ago
            let events = DbEvent::fetch(Some(
                &format!(" kind=1 AND created_at > {} ORDER BY created_at ASC", then)
            )).await?;

            // FIXME: build event_metadata as we go and send a 'setmetadata'
            //        with them.

            tx.send(BusMessage {
                target: "to_javascript".to_string(),
                source: "overlord".to_string(),
                kind: "pushfeedevents".to_string(),
                payload: serde_json::to_string(&events)?,
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

        // Load person data from database and send to javascript
        // FIXME: in the future when we have lots of people, JavaScript can ask first.
        {
            let people = DbPerson::fetch(None).await?;
            tx.send(BusMessage {
                target: "to_javascript".to_string(),
                source: "overlord".to_string(),
                kind: "setpeople".to_string(),
                payload: serde_json::to_string(&people)?
            })?;
        }

        // Start a thread for each relay
        for (url, filters) in relay_filters.iter() {
            let task_filters: Filters = filters.clone();
            let task_url: Url = url.clone();

            // We don't really need to keep the abort_handle as we will use
            // the JoinSet variable which is more powerful. But it has the
            // task id, so that's convenient.
            let abort_handle = relay_tasks.spawn(async move {
                let websocket_handler = crate::nostr::WebsocketHandler::new(
                    task_url,
                    task_filters
                );
                websocket_handler.handle().await
            });
            let id = abort_handle.id();

            task_id_to_relay_url.insert(id, url.clone());
        }

        'mainloop: loop {
            if relay_tasks.is_empty() {
                // We only need to listen on the bus
                let bus_message = rx.recv().await.unwrap();
                let keepgoing = self.handle_bus_message(bus_message);
                if !keepgoing { break 'mainloop; }
            } else {
                // We need to listen on the bus, and for completed tasks
                select! {
                    bus_message = rx.recv() => {
                        let bus_message = bus_message.unwrap();
                        let keepgoing = self.handle_bus_message(bus_message);
                        if !keepgoing { break 'mainloop; }
                    },
                    task_next_joined = relay_tasks.join_next_with_id() => {
                        if task_next_joined.is_none() { continue; } // rare
                        match task_next_joined.unwrap() {
                            Err(join_error) => {
                                let id = join_error.id();
                                let relay_url = task_id_to_relay_url.get(&id);
                                match relay_url {
                                    Some(url) => {
                                        // JoinError also has is_cancelled, is_panic, into_panic, try_into_panic
                                        log::warn!("Relay Task {} completed with error: {}", &url, join_error);
                                    },
                                    None => {
                                        log::warn!("Relay task UNKNOWN completed with error: {}", join_error);
                                    }
                                }
                            },
                            Ok((id, _)) => {
                                let relay_url = task_id_to_relay_url.get(&id);
                                match relay_url {
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
        }

        self.app_handle.exit(1);

        // TODO:
        // Figure out what relays we need to talk to
        // Start threads for each of them
        // Refigure it out and tell them

        Ok(())
    }

    fn handle_bus_message(&mut self, bus_message: BusMessage) -> bool {
        match &*bus_message.target {
            "to_javascript" => {
                if self.javascript_is_ready {
                    log::trace!(
                        "sending to javascript: kind={} payload={}",
                        bus_message.kind,
                        bus_message.payload
                    );
                    self.app_handle.emit_all("from_rust", bus_message).unwrap();
                } else {
                    log::debug!("PUSHING early message");
                    self.early_messages_to_javascript.push(bus_message);
                }
            }
            "all" => match &*bus_message.kind {
                "shutdown" => {
                    log::info!("Overlord shutting down");
                    return false;
                },
                _ => {}
            },
            "overlord" => match &*bus_message.kind {
                "javascript_is_ready" => {
                    log::info!("Javascript is ready");
                    self.javascript_is_ready = true;
                    self.send_early_messages_to_javascript();
                },
                _ => {}
            },
            _ => {}
        }
        true
    }

    fn send_early_messages_to_javascript(&mut self) {
        for bus_message in self.early_messages_to_javascript.drain(..) {
            log::debug!("POPPING early message");
            log::trace!(
                "sending to javascript: kind={} payload={}",
                bus_message.kind,
                bus_message.payload
            );
            self.app_handle.emit_all("from_rust", bus_message).unwrap();
        }
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
