#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[macro_use]
extern crate lazy_static;

use serde::Serialize;

/// Only one of these is ever created, via lazy_static!, and represents
/// global state for the rust application
pub struct Globals {
}

lazy_static! {
    /// Global state for the rust application:
    static ref GLOBALS: Globals = Globals {
    };
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(
            tauri::generate_handler![gossip_about]
        )
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
