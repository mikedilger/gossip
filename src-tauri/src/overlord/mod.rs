
use crate::{BusMessage, Error, GLOBALS};
use crate::db::{DbEvent, DbPerson, DbPersonRelay, DbRelay, DbSetting};
use nostr_proto::{IdHex, Metadata, PublicKeyHex, Unixtime, Url};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use tauri::{AppHandle, Manager};
use tokio::{select, task};
use tokio::sync::broadcast::{Sender, Receiver};

mod minion;
use minion::Minion;

mod relay_picker;
use relay_picker::{BestRelay, RelayPicker};

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

pub struct MinionRecord {
    pub relay_url: Url,
    pub ready: bool,
}

pub struct Overlord {
    app_handle: AppHandle,
    javascript_is_ready: bool,
    early_messages_to_javascript: Vec<BusMessage>,
    overlap: u64,
    feed_chunk: u64,
    bus_tx: Sender<BusMessage>,
    bus_rx: Receiver<BusMessage>,
    minions: task::JoinSet<()>,
    minion_records: HashMap<task::Id, MinionRecord>,
}

impl Overlord {
    pub fn new(app_handle: AppHandle) -> Overlord {
        let bus_tx = GLOBALS.bus.clone();
        let bus_rx = bus_tx.subscribe();
        Overlord {
            app_handle,
            javascript_is_ready: false,
            early_messages_to_javascript: Vec::new(),
            overlap: 0,
            feed_chunk: crate::DEFAULT_FEED_CHUNK,
            bus_tx, bus_rx,
            minions: task::JoinSet::new(),
            minion_records: HashMap::new(),
        }
    }

    pub async fn run(&mut self) {
        if let Err(e) = self.run_inner().await {
            log::error!("{}", e);
            if let Err(e) = self.bus_tx.send(BusMessage {
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

        // Setup the database (possibly create, possibly upgrade)
        setup_database().await?;

        // Read settings
        self.overlap = DbSetting::fetch_setting_u64_or_default("overlap", crate::DEFAULT_OVERLAP).await?;
        log::info!("OVERLAP={}", self.overlap);

        self.feed_chunk = DbSetting::fetch_setting_u64_or_default("feed_chunk", crate::DEFAULT_FEED_CHUNK).await?;
        log::info!("FEED_CHUNK={}", self.feed_chunk);
        let autofollow = DbSetting::fetch_setting_u64_or_default("autofollow", crate::DEFAULT_AUTOFOLLOW).await?;
        log::info!("AUTOFOLLOW={}", autofollow);

        // Create a person record for every person seen, possibly autofollow
        DbPerson::populate_new_people(autofollow!=0).await?;

        // Create a relay record for every relay in person_relay map (these get
        // updated from events without necessarily updating our relays list)
        DbRelay::populate_new_relays().await?;

        // Load TextNote event data from database and send to javascript
        {
            let now = Unixtime::now().unwrap();
            let then = now.0 - self.feed_chunk as i64;
            let events = DbEvent::fetch(Some(
                &format!(" kind=1 AND created_at > {} ORDER BY created_at ASC", then)
            )).await?;

            // FIXME: build event_metadata as we go and send a 'setmetadata'
            //        with them.

            self.bus_tx.send(BusMessage {
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

        // Load all people we are following
        let people = DbPerson::fetch(Some("followed=1")).await?;

        // Send these people to javascript
        self.bus_tx.send(BusMessage {
            target: "to_javascript".to_string(),
            source: "overlord".to_string(),
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
                let (rd, rp) = relay_picker.best()?;
                best_relay = rd;
                relay_picker = rp;

                // Fire off a minion to handle this relay
                {
                    let url = Url(best_relay.relay.url.clone());
                    let pubkeys = best_relay.pubkeys.clone();
                    let abort_handle = self.minions.spawn(async move {
                        let mut minion = Minion::new(url, pubkeys);
                        minion.handle().await
                    });
                    let id = abort_handle.id();

                    self.minion_records.insert(id, MinionRecord {
                        relay_url: Url(best_relay.relay.url.clone()),
                        ready: false
                    });
                }

                log::info!("Picked relay {}, {} people left",
                           best_relay.relay.url,
                           relay_picker.pubkeys.len());

                if relay_picker.relays.len()==0 { break; }
                if relay_picker.pubkeys.len()==0 { break; }
            }
        }

        'mainloop: loop {
            if self.minions.is_empty() {
                // We only need to listen on the bus
                let bus_message = self.bus_rx.recv().await.unwrap();
                let keepgoing = self.handle_bus_message(bus_message);
                if !keepgoing { break 'mainloop; }
            } else {
                // We need to listen on the bus, and for completed tasks
                select! {
                    bus_message = self.bus_rx.recv() => {
                        let bus_message = bus_message.unwrap();
                        let keepgoing = self.handle_bus_message(bus_message);
                        if !keepgoing { break 'mainloop; }
                    },
                    task_next_joined = self.minions.join_next_with_id() => {
                        if task_next_joined.is_none() { continue; } // rare
                        match task_next_joined.unwrap() {
                            Err(join_error) => {
                                let id = join_error.id();
                                let maybe_minion_record = self.minion_records.get(&id);
                                match maybe_minion_record {
                                    Some(minion_record) => {
                                        // JoinError also has is_cancelled, is_panic, into_panic, try_into_panic
                                        log::warn!("Minion {} completed with error: {}", &minion_record.relay_url, join_error);
                                    },
                                    None => {
                                        log::warn!("Minion UNKNOWN completed with error: {}", join_error);
                                    }
                                }
                            },
                            Ok((id, _)) => {
                                let maybe_minion_record = self.minion_records.get(&id);
                                match maybe_minion_record {
                                    Some(minion_record) => log::warn!("Relay Task {} completed", &minion_record.relay_url),
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
