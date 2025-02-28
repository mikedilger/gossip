#![cfg_attr(not(debug_assertions), windows_subsystem = "console")]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
// TEMPORARILY
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::assigning_clones)]

mod about;

use gossip_lib::Error;
use std::env;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

fn main() -> Result<(), Error> {
    setup_logging();

    let about = about::About::new();
    tracing::info!("Gossip-GTK {}", about.version);

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
