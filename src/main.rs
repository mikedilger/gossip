#[macro_use]
extern crate lazy_static;

mod db;

mod comms;
use comms::BusMessage;

mod error;
use error::Error;

mod settings;

mod ui;

use rusqlite::Connection;
use std::{env, thread};
use tokio::sync::{broadcast, mpsc, Mutex};

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

fn main() {
    // Set up logging
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    if env::var("RUST_LOG_STYLE").is_err() {
        env::set_var("RUST_LOG_STYLE", "auto");
    }
    env_logger::init();

    // Create a separate thread to drive the User Interface
    let ui_thread = thread::spawn(move || {
        ui::run();
    });

    // The main thread will be driven by tokio
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(tokio_main());

    // Don't exit until the UI thread exits
    let _ = ui_thread.join();
}

async fn tokio_main() {
    println!("Hello World");
}
