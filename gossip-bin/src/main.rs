#![cfg_attr(not(debug_assertions), windows_subsystem = "console")]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
// TEMPORARILY
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::assigning_clones)]

mod about;
mod commands;
mod date_ago;
mod ui;
mod unsaved_settings;

use gossip_lib::{Error, RunState, GLOBALS};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{env, thread};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::prelude::*;

pub const AVATAR_SIZE: u32 = 48; // points, not pixels
pub const AVATAR_SIZE_F32: f32 = 48.0; // points, not pixels
pub const AVATAR_SIZE_REPOST: u32 = 27; // points, not pixels
pub const AVATAR_SIZE_REPOST_F32: f32 = 27.0; // points, not pixels

fn main() -> Result<(), Error> {
    // Read command line parameters
    let mut rapid: bool = false;
    let mut debug_async: bool = false;
    {
        let mut args = env::args();
        let _ = args.next(); // program name
        for cmd in args {
            if &*cmd == "--rapid" {
                rapid = true;
            }
            if &*cmd == "--debug-async" {
                debug_async = true;
            }
        }
    }

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

    if debug_async {
        let console_layer = console_subscriber::spawn();

        let main_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_target(false)
            .with_file(show_debug)
            .with_line_number(show_debug)
            .with_filter(env_filter);

        tracing_subscriber::registry()
            .with(console_layer)
            .with(main_layer)
            .init();
    } else {
        tracing_subscriber::fmt::fmt()
            .with_writer(std::io::stderr)
            .with_target(false)
            .with_file(show_debug)
            .with_line_number(show_debug)
            .with_env_filter(env_filter)
            .init();
    }

    let about = about::About::new();
    tracing::info!("Gossip {}", about.version);

    // restart args
    let mut args = env::args();
    let _ = args.next(); // program name
    if rapid {
        let _ = args.next(); // --rapid param
    }
    if debug_async {
        let _ = args.next(); // --debug-async param
    }

    // Setup async, and allow non-async code the context to spawn tasks
    let _main_rt = GLOBALS.runtime.enter(); // <-- this allows it.

    // Initialize the lib
    GLOBALS
        .runtime
        .block_on(async { gossip_lib::init(rapid, args.len() > 0).await })?;

    // If we were handed a command, execute the command and (usually) return
    let exit: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let exit2 = exit.clone();
    if args.len() > 0 {
        GLOBALS.runtime.block_on(async {
            let exit = match commands::handle_command(args).await {
                Err(e) => {
                    println!("{}", e);
                    true
                }
                Ok(exit) => exit,
            };
            if exit {
                exit2.store(true, Ordering::Relaxed);
            }
        });

        if exit.load(Ordering::Relaxed) {
            return Ok(());
        }
    }

    // We run our main async code on a separate thread, not just a
    // separate task. This leave the main thread for UI work only.
    // egui is most portable when it is on the main thread.
    let async_thread = thread::spawn(move || {
        GLOBALS.runtime.block_on(Box::pin(gossip_lib::run()));
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
