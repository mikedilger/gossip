#![cfg_attr(not(debug_assertions), windows_subsystem = "console")]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
// TEMPORARILY
#![allow(clippy::uninlined_format_args)]

mod commands;
mod date_ago;
mod ui;
mod unsaved_settings;

use gossip_lib::Error;
use gossip_lib::GLOBALS;
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
        .with_target(false)
        .with_file(show_debug)
        .with_line_number(show_debug)
        .with_env_filter(env_filter)
        .init();

    // Initialize the lib
    gossip_lib::init()?;

    // Setup async
    // We create and enter the runtime on the main thread so that
    // non-async code can have a runtime context within which to spawn
    // async tasks.
    let rt = tokio::runtime::Runtime::new()?;
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
        rt.block_on(gossip_lib::run());
    });

    // Run the UI
    if let Err(e) = ui::run() {
        tracing::error!("{}", e);
    }

    // Make sure the overlord knows to shut down
    GLOBALS.shutting_down.store(true, Ordering::Relaxed);

    // Make sure the overlord isn't stuck on waiting for login
    GLOBALS.wait_for_login.store(false, Ordering::Relaxed);
    GLOBALS.wait_for_login_notify.notify_one();

    // Tell the async parties to close down
    GLOBALS.shutting_down.store(true, Ordering::Relaxed);

    // Wait for the async thread to complete
    async_thread.join().unwrap();

    gossip_lib::shutdown()?;

    Ok(())
}
