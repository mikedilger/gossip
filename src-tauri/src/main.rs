#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[macro_use]
extern crate lazy_static;

use rusqlite::Connection;
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
#[derive(Debug, Clone)]
pub struct Message {
    /// Who the message is for
    pub target: String,

    /// What the message is
    pub message: String,
}

/// Only one of these is ever created, via lazy_static!, and represents
/// global state for the rust application
pub struct Globals {
    /// This is our connection to SQLite. Only one thread at a time.
    pub db: Mutex<Option<Connection>>,

    /// This is a broadcast channel. All tasks can listen on it.
    /// To create a receiver, just run .subscribe() on it.
    pub broadcast: broadcast::Sender<Message>,
}

lazy_static! {
    static ref GLOBALS: Globals = {

        // Setup a watch communications channel (single-producer, multiple watchers)
        // that we can use to signal our websocket driving threads about changes
        // to what they should be following, events we need to send, etc.
        let (broadcast_tx, _) = broadcast::channel(16);

        Globals {
            db: Mutex::new(None),
            broadcast: broadcast_tx
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
        .invoke_handler(
            tauri::generate_handler![commands::about]
        )
        .setup(|app| {
            let app_handle = app.handle();
            let _join = tauri::async_runtime::spawn(
                mainloop(app_handle)
            );
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    println!("This is never printed. Tauri exits.");
}

async fn mainloop(app_handle: AppHandle) {

    // Get the broadcast channel and subscribe to it
    let tx = GLOBALS.broadcast.clone();
    let mut rx = tx.subscribe();

    // Setup the database (possibly create, possibly upgrade)
    if let Err(e) = setup_database().await {
        log::error!("{}", e);
        if let Err(e) = tx.send(Message {
            target: "all".to_string(),
            message: "shutdown".to_string()
        }) {
            log::error!("Unable to send message: {}", e);
        }
        return;
    }

    // Wait until Taori is up
    // TBD - this is a hack. Listen for an event instead.
    tokio::time::sleep(std::time::Duration::new(1,0)).await;

    // Send a first message to javascript (actually to us, then we send it
    // onwards -- we read our own message just below)
    if let Err(e) = tx.send(Message {
        target: "to_javascript".to_string(),
        message: "Hello World".to_string()
    }) {
        log::error!("Unable to send message: {}", e);
    }

    'mainloop:
    loop {
        let message = rx.recv().await.unwrap();
        match &*message.target {
            "to_javascript" => {
                log::info!("sending to javascript: {}", message.message);
                app_handle
                    .emit_all("from_rust", message.message)
                    .unwrap();
            },
            "all" => {
                match &*message.message {
                    "shutdown" => {
                        break 'mainloop;
                    },
                    _ => {}
                }
            }
            _ => { }
        }
        // TBD: handle other messages
    }

    // TODO:
    // Figure out what relays we need to talk to
    // Start threads for each of them
    // Refigure it out and tell them
}

// This sets up the database
async fn setup_database() -> Result<(), Error> {
    let mut data_dir = dirs::data_dir().ok_or::<Error>(
        From::from("Cannot find a directory to store application data.")
    )?;
    data_dir.push("gossip");

    // Create our data directory only if it doesn't exist
    fs::create_dir_all(&data_dir)?;

    // Connect to (or create) our database
    let mut db_path = data_dir.clone();
    db_path.push("gossip.sqlite");
    let connection =  Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE |
        rusqlite::OpenFlags::SQLITE_OPEN_CREATE |
        rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX |
        rusqlite::OpenFlags::SQLITE_OPEN_NOFOLLOW)?;

    // Save the connection globally
    {
        let mut db = GLOBALS.db.lock().await;
        *db = Some(connection);
    }

    // Check and upgrade our data schema
    crate::db::check_and_upgrade().await?;

    Ok(())
}
