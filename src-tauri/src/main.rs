#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[macro_use]
extern crate lazy_static;

use rusqlite::Connection;
use serde::Serialize;
use std::env;
use std::fs;
use tokio::sync::Mutex;

mod error;
pub use error::Error;

/// Only one of these is ever created, via lazy_static!, and represents
/// global state for the rust application
pub struct Globals {
    pub db: Mutex<Option<Connection>>,
}

lazy_static! {
    /// Global state for the rust application:
    static ref GLOBALS: Globals = Globals {
        db: Mutex::new(None),
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

    tauri::Builder::default()
        .invoke_handler(
            tauri::generate_handler![gossip_about]
        )
        .setup(|_app| {
            // This will be our main asynchronous rust thread
            let _join = tauri::async_runtime::spawn(async move {
                // Setup the database (possibly create, possibly upgrade)
                setup_database().await?;

                // return type hint for this async block: Result<(), Error>
                // NOTE: the caller is dropping this error on the floor currently.
                Ok::<(), Error>(())
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[derive(Debug, Serialize)]
pub struct About {
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: String,
    pub repository: String,
    pub homepage: String,
}

#[tauri::command]
fn gossip_about() -> About {
    About {
        name: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: env!("CARGO_PKG_DESCRIPTION").to_string(),
        authors: env!("CARGO_PKG_AUTHORS").to_string(),
        repository: env!("CARGO_PKG_REPOSITORY").to_string(),
        homepage: env!("CARGO_PKG_HOMEPAGE").to_string(),
    }
}

// This sets up the database
async fn setup_database() -> Result<(), Error> {
    let mut data_dir = dirs::data_dir().ok_or::<Error>(
        From::from("Cannot find a directory to store application data.")
    )?;
    data_dir.push("gossip");

    // Create our data directory only if it doesn't exist
    fs::create_dir_all(&data_dir)?;

    // Connect to (or create) our database
    let mut db_path = data_dir.clone();
    db_path.push("gossip.sqlite");
    let connection =  Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE |
        rusqlite::OpenFlags::SQLITE_OPEN_CREATE |
        rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX |
        rusqlite::OpenFlags::SQLITE_OPEN_NOFOLLOW)?;

    // Save the connection globally
    {
        let mut db = GLOBALS.db.lock().await;
        *db = Some(connection);
    }

    Ok(())
}
