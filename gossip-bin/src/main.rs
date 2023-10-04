#![cfg_attr(not(debug_assertions), windows_subsystem = "console")]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
// TEMPORARILY
#![allow(clippy::uninlined_format_args)]

mod commands;
mod ui;

use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::error::Error;
use gossip_lib::globals::GLOBALS;
use std::ops::DerefMut;
use std::{env, thread};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

pub const AVATAR_SIZE: u32 = 48; // points, not pixels
pub const AVATAR_SIZE_F32: f32 = 48.0; // points, not pixels

fn main() -> Result<(), Error> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    let env_filter = EnvFilter::from_default_env();
    let max_level = match env_filter.max_level_hint() {
        Some(l) => l,
        None => LevelFilter::ERROR,
    };
    let show_debug = cfg!(debug_assertions) || max_level <= LevelFilter::DEBUG;
    tracing_subscriber::fmt::fmt()
        .with_target(false)
        .with_file(show_debug)
        .with_line_number(show_debug)
        .with_env_filter(env_filter)
        .init();

    // Initialize storage
    GLOBALS.storage.init()?;

    // We create and enter the runtime on the main thread so that
    // non-async code can have a runtime context within which to spawn
    // async tasks.
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _main_rt = rt.enter(); // <-- this allows it.

    // If we were handed a command, execute the command and return
    let args = env::args();
    if args.len() > 1 {
        match commands::handle_command(args, &rt) {
            Err(e) => {
                println!("{}", e);
                return Ok(());
            }
            Ok(exit) => {
                if exit {
                    return Ok(());
                }
            }
        }
    }

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

    // Sync storage again
    if let Err(e) = GLOBALS.storage.sync() {
        tracing::error!("{}", e);
    } else {
        tracing::info!("LMDB synced.");
    }

    Ok(())
}

async fn tokio_main() {
    // Steal `tmp_overlord_receiver` from the GLOBALS, and give it to a new Overlord
    let overlord_receiver = {
        let mut mutex_option = GLOBALS.tmp_overlord_receiver.lock().await;
        mutex_option.deref_mut().take()
    }
    .unwrap();

    // Run the overlord
    let mut overlord = gossip_lib::overlord::Overlord::new(overlord_receiver);
    overlord.run().await;
}

// Any task can call this to shutdown
pub fn initiate_shutdown() -> Result<(), Error> {
    let to_overlord = GLOBALS.to_overlord.clone();
    let _ = to_overlord.send(ToOverlordMessage::Shutdown); // ignore errors
    Ok(())
}
