#![cfg_attr(not(debug_assertions), windows_subsystem = "console")]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
// TEMPORARILY
#![allow(clippy::uninlined_format_args)]

//! Gossip lib is the core of gossip.  The canonical binary crate is `gossip_bin`.
//! This library has been separated so that people can attach different non-canonical
//! user interfaces on top of this core.
//!
//! Because of the history of this API, it may be a bit clunky. But we will work to
//! improve that. Please submit PRs if you want to help. This interface will change.
//! fairly rapidly for a while and then settle down.
//!
//! Further general documentation TBD.

mod about;
pub use about::About;

/// Defines messages sent to the overlord
pub mod comms;

mod delegation;
pub use delegation::Delegation;

mod dm_channel;
pub use dm_channel::{DmChannel, DmChannelData};

mod error;
pub use error::{Error, ErrorKind};

mod feed;
pub use feed::{Feed, FeedKind};

mod fetcher;
pub use fetcher::Fetcher;

mod filter;

mod globals;
pub use globals::{Globals, ZapState, GLOBALS};

mod media;
pub use media::Media;

/// Rendering various names of users
pub mod names;

/// nip05 handling
pub mod nip05;

mod overlord;
pub use overlord::Overlord;

mod people;
pub use people::{Person, PersonList};

mod person_relay;
pub use person_relay::PersonRelay;

/// Processing incoming events
pub mod process;

mod profile;

mod relationship;

mod relay;
pub use relay::Relay;

mod relay_picker_hooks;

mod settings;
pub use settings::Settings;

mod signer;
pub use signer::Signer;

mod status;

mod storage;
pub use storage::types::*;
pub use storage::Storage;

mod tags;

#[macro_use]
extern crate lazy_static;

/// The USER_AGENT string for gossip that it (may) use when fetching HTTP resources and
/// when connecting to relays
pub static USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
