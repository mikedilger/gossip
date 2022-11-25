#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[macro_use]
extern crate lazy_static;

use rusqlite::Connection;
use serde::Serialize;
use std::env;
use tokio::sync::broadcast;
use tokio::sync::Mutex;

mod commands;

mod db;

mod error;
pub use error::Error;

mod nostr;

mod overlord;

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

const DEFAULT_FEED_CHUNK: u64 = 43200; // 12 hours
const DEFAULT_OVERLAP: u64 = 600; // 10 minutes

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
        .invoke_handler(tauri::generate_handler![
            commands::about,
            commands::javascript_is_ready
        ])
        .setup(|app| {
            let app_handle = app.handle();
            let _join = tauri::async_runtime::spawn(async move {
                let mut overlord = crate::overlord::Overlord::new(app_handle);
                overlord.run().await;
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    println!("This is never printed. Tauri exits.");
}
