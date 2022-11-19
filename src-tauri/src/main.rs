#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[macro_use]
extern crate lazy_static;

use nostr_proto::{Filters, Url};
use rusqlite::Connection;
use serde::Serialize;
use std::env;
use std::fs;
use tauri::{AppHandle, Manager};
use tokio::sync::broadcast;
use tokio::sync::Mutex;

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

    // Wait until Taori is up
    // TBD - this is a hack. Listen for an event instead.
    tokio::time::sleep(std::time::Duration::new(1, 0)).await;

    // Send a first message to javascript (actually to us, then we send it
    // onwards -- we read our own message just below)
    if let Err(e) = tx.send(BusMessage {
        target: "to_javascript".to_string(),
        source: "mainloop".to_string(),
        kind: "greeting".to_string(),
        payload: serde_json::to_string("Hello World").unwrap(),
    }) {
        log::error!("Unable to send message: {}", e);
    }

    // Load the initial relay filters
    let mut relay_filters = match crate::nostr::load_initial_relay_filters().await {
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

    // Start a thread for each relay
    for (url, filters) in relay_filters.iter_mut() {
        let task_filters: Filters = filters.clone();
        let task_url: Url = url.clone();

        // We don't need a join handle. And we can broadcast to it once it
        // starts listening to broadcast
        tauri::async_runtime::spawn(async move {
            crate::nostr::handle_relay(task_filters, task_url).await
        });
    }

    'mainloop: loop {
        let message = rx.recv().await.unwrap();
        match &*message.target {
            "to_javascript" => {
                log::info!(
                    "sending to javascript: kind={} payload={}",
                    message.kind,
                    message.payload
                );
                app_handle.emit_all("from_rust", message).unwrap();
            }
            "all" => match &*message.kind {
                "shutdown" => {
                    log::info!("Mainloop shutting down");
                    break 'mainloop;
                }
                _ => {}
            },
            _ => {}
        }
        // TBD: handle other messages
    }

    app_handle.exit(1);

    // TODO:
    // Figure out what relays we need to talk to
    // Start threads for each of them
    // Refigure it out and tell them
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
