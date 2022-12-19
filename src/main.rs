#[macro_use]
extern crate lazy_static;

mod comms;
mod db;
mod error;
mod event_related;
mod globals;
mod overlord;
mod settings;
mod ui;

use comms::BusMessage;
use error::Error;
use event_related::EventRelated;
use globals::GLOBALS;
use std::ops::DerefMut;
use std::{env, thread};

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
        // TEMPORARY HACK SO THE LIST IS CREATED AFTER THE FEED IS LOADED
        // Later, use the ListModel::items-changed signal
        std::thread::sleep(std::time::Duration::from_millis(1000));
        ui::run();
    });

    // The main thread will be driven by tokio
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(tokio_main());

    // Don't exit until the UI thread exits
    let _ = ui_thread.join();
}

async fn tokio_main() {
    // Steal `from_minions` from the GLOBALS, and give it to a new Overlord
    let from_minions = {
        let mut mutex_option = GLOBALS.from_minions.lock().await;
        std::mem::replace(mutex_option.deref_mut(), None)
    }
    .unwrap();

    // Run the overlord
    let mut overlord = crate::overlord::Overlord::new(from_minions);
    overlord.run().await;
}

// Any task can call this to shutdown
pub fn initiate_shutdown() -> Result<(), Error> {
    let to_overlord = GLOBALS.to_overlord.clone();
    to_overlord.send(BusMessage {
        target: "all".to_string(),
        kind: "shutdown".to_string(),
        json_payload: serde_json::to_string("").unwrap(),
    })?;
    Ok(())
}
