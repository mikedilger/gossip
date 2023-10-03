#![cfg_attr(not(debug_assertions), windows_subsystem = "console")]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
// TEMPORARILY
#![allow(clippy::uninlined_format_args)]

pub mod about;
pub mod commands;
pub mod comms;
pub mod date_ago;
pub mod delegation;
pub mod dm_channel;
pub mod error;
pub mod feed;
pub mod fetcher;
pub mod filter;
pub mod globals;
pub mod media;
pub mod names;
pub mod nip05;
pub mod overlord;
pub mod people;
pub mod person_relay;
pub mod process;
pub mod profile;
pub mod relationship;
pub mod relay;
pub mod relay_picker_hooks;
pub mod settings;
pub mod signer;
pub mod status;
pub mod storage;
pub mod tags;
pub mod ui;

#[macro_use]
extern crate lazy_static;

pub const AVATAR_SIZE: u32 = 48; // points, not pixels
pub const AVATAR_SIZE_F32: f32 = 48.0; // points, not pixels

pub static USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
