#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

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

use crate::comms::BusMessage;
use crate::error::Error;
use crate::globals::GLOBALS;
use std::ops::DerefMut;
use std::{mem, thread};

fn main() {
    tracing_subscriber::fmt::init();

    // Start async code
    // We do this on a separate thread because egui is most portable by
    // being on the main thread.
    let async_thread = thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(tokio_main());
    });

    if let Err(e) = ui::run() {
        tracing::error!("{}", e);
    }

    // Tell the async parties to close down
    if let Err(e) = initiate_shutdown() {
        tracing::error!("{}", e);
    }

    // Wait for the async thread to complete
    async_thread.join().unwrap();
}

async fn tokio_main() {
    // Steal `from_minions` from the GLOBALS, and give it to a new Overlord
    let from_minions = {
        let mut mutex_option = GLOBALS.from_minions.lock().await;
        mem::replace(mutex_option.deref_mut(), None)
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
