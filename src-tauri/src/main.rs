#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[macro_use]
extern crate lazy_static;

use rusqlite::Connection;
use serde::Serialize;
use std::env;
use std::ops::DerefMut;
use tokio::sync::{broadcast, mpsc};
use tokio::sync::Mutex;

mod commands;

mod db;

mod error;
pub use error::Error;

mod overlord;

mod settings;
pub use settings::Settings;

/// This is a message sent between the Overlord and Minions
/// in either direction
#[derive(Debug, Clone, Serialize)]
pub struct BusMessage {
    /// Who the message is for or from (depending on the direction),
    /// Not required for 'all' or 'javascript' messages.
    pub relay_url: Option<String>,

    /// 'javascript', 'overlord', 'all', or 'relay_url'
    pub target: String,

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

    /// This is a broadcast channel. All Minions should listen on it.
    /// To create a receiver, just run .subscribe() on it.
    pub to_minions: broadcast::Sender<BusMessage>,

    /// This is a mpsc channel. The Overlord listens on it.
    /// To create a sender, just clone() it.
    pub to_overlord: mpsc::UnboundedSender<BusMessage>,

    /// This is ephemeral. It is filled during lazy_static initialization,
    /// and stolen away when the Overlord is created.
    pub from_minions: Mutex<Option<mpsc::UnboundedReceiver<BusMessage>>>,
}

lazy_static! {
    static ref GLOBALS: Globals = {

        // Setup a communications channel from the Overlord to the Minions.
        let (to_minions, _) = broadcast::channel(16);

        // Setup a communications channel from the Minions to the Overlord.
        let (to_overlord, from_minions) = mpsc::unbounded_channel();

        Globals {
            db: Mutex::new(None),
            to_minions,
            to_overlord,
            from_minions: Mutex::new(Some(from_minions)),
        }
    };
}

const DEFAULT_FEED_CHUNK: u64 = 43200; // 12 hours
const DEFAULT_OVERLAP: u64 = 600; // 10 minutes
const DEFAULT_AUTOFOLLOW: u64 = 0;

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
            commands::javascript_is_ready,
            commands::save_settings,
        ])
        .setup(|app| {
            let app_handle = app.handle();
            let _join = tauri::async_runtime::spawn(async move {

                // Steal from_minions from the GLOBALS, and give it to the Overlord
                let from_minions = {
                    let mut mutex_option = GLOBALS.from_minions.lock().await;
                    std::mem::replace(mutex_option.deref_mut(), None)
                }.unwrap();

                let mut overlord = crate::overlord::Overlord::new(app_handle, from_minions);
                overlord.run().await;
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    println!("This is never printed. Tauri exits.");
}
