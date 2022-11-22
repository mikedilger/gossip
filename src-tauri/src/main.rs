#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[macro_use]
extern crate lazy_static;

use nostr_proto::{Filters, Url};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use tauri::{AppHandle, Manager};
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tokio::{select, task};

mod commands;

mod db;

mod error;
pub use error::Error;

mod nostr;

/// This is a message we send/recv in our broadcast channel
#[derive(Debug, Clone, Serialize)]
pub struct BusMessage {
    /// Who the message is for
    pub target: String,

    /// Who is the message from
    pub source: String,

    /// What kind of message is this
    pub kind: String,

    /// What is the payload
    pub payload: String,
}

/// Only one of these is ever created, via lazy_static!, and represents
/// global state for the rust application
pub struct Globals {
    /// This is our connection to SQLite. Only one thread at a time.
    pub db: Mutex<Option<Connection>>,

    /// This is a broadcast channel. All tasks can listen on it.
    /// To create a receiver, just run .subscribe() on it.
    pub bus: broadcast::Sender<BusMessage>,
}

lazy_static! {
    static ref GLOBALS: Globals = {

        // Setup a watch communications channel (single-producer, multiple watchers)
        // that we can use to signal our websocket driving threads about changes
        // to what they should be following, events we need to send, etc.
        let (broadcast_tx, _) = broadcast::channel(256);

        Globals {
            db: Mutex::new(None),
            bus: broadcast_tx
        }
    };
}

fn main() {
    // Set up logging
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    if env::var("RUST_LOG_STYLE").is_err() {
        env::set_var("RUST_LOG_STYLE", "auto");
    }
    env_logger::init();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::about])
        .setup(|app| {
            let app_handle = app.handle();
            let _join = tauri::async_runtime::spawn(mainloop(app_handle));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    println!("This is never printed. Tauri exits.");
}

async fn mainloop(app_handle: AppHandle) {
    // Get the broadcast channel and subscribe to it
    let tx = GLOBALS.bus.clone();
    let mut rx = tx.subscribe();

    // Setup the database (possibly create, possibly upgrade)
    if let Err(e) = setup_database().await {
        log::error!("{}", e);
        if let Err(e) = tx.send(BusMessage {
            target: "all".to_string(),
            source: "mainloop".to_string(),
            kind: "shutdown".to_string(),
            payload: "shutdown".to_string(),
        }) {
            log::error!("Unable to send message: {}", e);
        }
        app_handle.exit(1);
        return;
    }

    // Load the initial relay filters
    let relay_filters = match crate::nostr::load_initial_relay_filters().await {
        Ok(rf) => rf,
        Err(e) => {
            log::error!("Could not load initial relay filters: {}", e);
            if let Err(e) = tx.send(BusMessage {
                target: "all".to_string(),
                source: "mainloop".to_string(),
                kind: "shutdown".to_string(),
                payload: "shutdown".to_string(),
            }) {
                log::error!("Unable to send message: {}", e);
            }
            app_handle.exit(1);
            return;
        }
    };

    // Wait until Taori is up
    // TBD - this is a hack. Listen for an event instead.
    tokio::time::sleep(std::time::Duration::new(1, 0)).await;

    // Keep the join handles for the relay tasks
    let mut relay_tasks = task::JoinSet::new();
    // Keep a mapping from Task ID to relay url
    let mut task_id_to_relay_url: HashMap<task::Id, Url> = HashMap::new();

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
            let keepgoing = handle_bus_message(bus_message, app_handle.clone());
            if !keepgoing { break 'mainloop; }
        } else {
            // We need to listen on the bus, and for completed tasks
            select! {
                bus_message = rx.recv() => {
                    let bus_message = bus_message.unwrap();
                    let keepgoing = handle_bus_message(bus_message, app_handle.clone());
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
                                    log::warn!("Relay Task {} completed with error: {}", &url.0, join_error);
                                },
                                None => {
                                    log::warn!("Relay task UNKNOWN completed with error: {}", join_error);
                                }
                            }
                        },
                        Ok((id, _)) => {
                            let relay_url = task_id_to_relay_url.get(&id);
                            match relay_url {
                                Some(url) => log::warn!("Relay Task {} completed", &url.0),
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

    app_handle.exit(1);

    // TODO:
    // Figure out what relays we need to talk to
    // Start threads for each of them
    // Refigure it out and tell them
}

fn handle_bus_message(bus_message: BusMessage, app_handle: AppHandle) -> bool {
    match &*bus_message.target {
        "to_javascript" => {
            log::trace!(
                "sending to javascript: kind={} payload={}",
                bus_message.kind,
                bus_message.payload
            );
            app_handle.emit_all("from_rust", bus_message).unwrap();
        }
        "all" => match &*bus_message.kind {
            "shutdown" => {
                log::info!("Mainloop shutting down");
                return false;
            }
            _ => {}
        },
        _ => {}
    }
    true
}

// This sets up the database
async fn setup_database() -> Result<(), Error> {
    let mut data_dir = dirs::data_dir().ok_or::<Error>(From::from(
        "Cannot find a directory to store application data.",
    ))?;
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
