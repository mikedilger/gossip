#![cfg_attr(not(debug_assertions), windows_subsystem = "console")]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
// TEMPORARILY
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::assigning_clones)]

mod about;
mod commands;

use gossip_lib::{Error, GLOBALS};
use std::env;
use std::env::Args;
use std::iter::Peekable;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

fn main() -> Result<(), Error> {
    setup_logging();

    let about = about::About::new();
    tracing::info!("Gossip-GTK {}", about.version);

    // Handle rapid command before initializing the lib
    let (rapid, args) = check_rapid();

    // Initialize the lib
    let command_mode = args.len() > 0;
    gossip_lib::init(rapid, command_mode)?;

    // Setup async, and allow non-async code the context to spawn tasks
    let _main_rt = GLOBALS.runtime.enter(); // <-- this allows it.

    // If we were handed a command, execute the command and return
    if command_mode {
        match commands::handle_command(args) {
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

    tracing::error!("TBD");

    Ok(())
}

fn setup_logging() {
    // Setup logging
    let env_filter = if env::var("RUST_LOG").is_err() {
        EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .from_env_lossy()
    } else {
        EnvFilter::from_default_env()
    };

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
}

fn check_rapid() -> (bool, Peekable<Args>) {
    let mut rapid: bool = false;
    let mut args = env::args().peekable();
    let _ = args.next(); // program name
    if let Some(cmd) = args.peek().cloned() {
        if &*cmd == "rapid" || &*cmd == "--rapid" {
            let _ = args.next();
            rapid = true;
        }
    }
    (rapid, args)
}
