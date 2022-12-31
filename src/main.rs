#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

#[macro_use]
extern crate lazy_static;

mod about;
mod comms;
mod date_ago;
mod db;
mod error;
mod feed;
mod fetcher;
mod globals;
mod overlord;
mod people;
mod process;
mod relationship;
mod settings;
mod signer;
mod syncer;
mod ui;

use crate::comms::BusMessage;
use crate::error::Error;
use crate::globals::GLOBALS;
use std::ops::DerefMut;
use std::{env, mem, thread};
use tracing_subscriber::filter::EnvFilter;

fn main() -> Result<(), Error> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    tracing_subscriber::fmt::fmt()
        .without_time()
        .with_file(cfg!(debug_assertions))
        .with_line_number(cfg!(debug_assertions))
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Setup the database (possibly create, possibly upgrade)
    crate::db::setup_database()?;

    // Load settings
    let settings = crate::settings::Settings::blocking_load()?;
    *GLOBALS.settings.blocking_write() = settings;

    // We create and enter the runtime on the main thread so that
    // non-async code can have a runtime context within which to spawn
    // async tasks.
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _main_rt = rt.enter(); // <-- this allows it.

    // We run our main async code on a separate thread, not just a
    // separate task. This leave the main thread for UI work only.
    // egui is most portable when it is on the main thread.
    let async_thread = thread::spawn(move || {
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

    Ok(())
}

async fn tokio_main() {
    // note: we don't get a spawn handle here, we don't signal this thread when we are exiting,
    // we just let it die when tokio_main() exits. I think that is ok.
    tokio::task::spawn(async move {
        // Steal `tmp_syncer_receiver` from the GLOBALS, and give it to a new Syncer
        let syncer_receiver = {
            let mut mutex_option = GLOBALS.tmp_syncer_receiver.lock().await;
            mem::replace(mutex_option.deref_mut(), None)
        }
        .unwrap();

        let mut syncer = crate::syncer::Syncer::new(syncer_receiver);
        syncer.run().await;
    });

    // Steal `tmp_overlord_receiver` from the GLOBALS, and give it to a new Overlord
    let overlord_receiver = {
        let mut mutex_option = GLOBALS.tmp_overlord_receiver.lock().await;
        mem::replace(mutex_option.deref_mut(), None)
    }
    .unwrap();

    // Run the overlord
    let mut overlord = crate::overlord::Overlord::new(overlord_receiver);
    overlord.run().await;
}

// Any task can call this to shutdown
pub fn initiate_shutdown() -> Result<(), Error> {
    let to_overlord = GLOBALS.to_overlord.clone();
    let _ = to_overlord.send(BusMessage {
        target: "all".to_string(),
        kind: "shutdown".to_string(),
        json_payload: serde_json::to_string("").unwrap(),
    }); // ignore errors
    Ok(())
}
