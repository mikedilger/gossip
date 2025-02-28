#![cfg_attr(not(debug_assertions), windows_subsystem = "console")]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
// TEMPORARILY
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::assigning_clones)]

mod about;
mod date_ago;
mod ui;
mod unsaved_settings;

use gossip_lib::{Error, RunState, GLOBALS};
use std::sync::atomic::Ordering;
use std::{env, thread};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

pub const AVATAR_SIZE: u32 = 48; // points, not pixels
pub const AVATAR_SIZE_F32: f32 = 48.0; // points, not pixels
pub const AVATAR_SIZE_REPOST_F32: f32 = 27.0; // points, not pixels

fn main() -> Result<(), Error> {
    // Setup logging
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
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_file(show_debug)
        .with_line_number(show_debug)
        .with_env_filter(env_filter)
        .init();

    let about = about::About::new();
    tracing::info!("Gossip {}", about.version);

    // Handle rapid command before initializing the lib
    let mut rapid: bool = false;
    {
        let mut args = env::args();
        let _ = args.next(); // program name
        if let Some(cmd) = args.next() {
            if &*cmd == "rapid" || &*cmd == "--rapid" {
                rapid = true;
            }
        }
    }

    // restart args
    let mut args = env::args();
    let _ = args.next(); // program name
    if rapid {
        let _ = args.next(); // rapid param
    }

    // Initialize the lib
    gossip_lib::init(rapid, args.len() > 0)?;

    // Setup async, and allow non-async code the context to spawn tasks
    let _main_rt = GLOBALS.runtime.enter(); // <-- this allows it.

    // If we were handed a command, execute the command and return
    if args.len() > 0 {
        match gossip_lib::commands::handle_command(args) {
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
        GLOBALS.runtime.block_on(gossip_lib::run());
    });

    // Run the UI
    if let Err(e) = ui::run() {
        tracing::error!("{}", e);
    }

    // Move to the ShuttingDown runstate
    let _ = GLOBALS.write_runstate.send(RunState::ShuttingDown);

    // Make sure the overlord isn't stuck on waiting for login
    GLOBALS.wait_for_login.store(false, Ordering::Relaxed);
    GLOBALS.wait_for_login_notify.notify_one();

    tracing::info!("UI thread complete, waiting on lib...");

    // Wait for the async thread to complete
    async_thread.join().unwrap();

    tracing::info!("Gossip end.");

    Ok(())
}
