#![cfg_attr(not(debug_assertions), windows_subsystem = "console")]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
// TEMPORARILY
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::assigning_clones)]

//! Gossip lib is the core of the gossip nostr client.  The canonical binary crate is
//! `gossip_bin`.
//!
//! This library has been separated so that people can attach different non-canonical
//! user interfaces on top of this core.
//!
//! Because of the history of this API, it may be a bit clunky. But we will work to
//! improve that. Please submit PRs if you want to help. This interface will change
//! fairly rapidly for a while and then settle down.
//!
//! # Using gossip-lib
//!
//! To use gossip-lib, depend on it in your Cargo.toml
//!
//! ```rust.ignore
//! gossip-lib = { git = "https://github.com/mikedilger/gossip" }
//! ```
//!
//! You may specify optional features including:
//!
//! * Choose between `rustls-tls` and `native-tls`
//! * `lang-cjk` to include Chinese, Japanese, and Korean fonts (which grow the size significantly)
//!
//! # Gossip Startup
//!
//! Gossip starts up in three phases.
//!
//! The first phase happens at static initialization.
//! The globally available GLOBALS variable is initialized when first accessed, lazily.
//! You don't have to do anything special to make this happen, and you can start using
//! `GLOBALS` whenever you wish.
//!
//! The second phase is where you have to initialize a few things such as `Storage::init()`.
//! There may be others.
//!
//! The third phase is creating and starting the `Overlord`. This needs to be spawned on
//! a rust async executor such as `tokio`. See [Overlord::new](crate::Overlord::new) for the
//! details of how to start it. The overlord will start anything else that needs starting,
//! and will manage connections to relays.
//!
//! # User Interfaces
//!
//! The canonical gossip user interface is egui-based, and is thus immediate mode. It runs on
//! the main thread and is not asynchronous. Every call it makes must return immediately so that
//! it can paint the next frame (which may happen rapidly when videos are playing or scrolling
//! is animating) and not stall the user experience. For this reason, the `Overlord` can be sent
//! messages through a global message queue `GLOBALS.to_overlord`.
//!
//! But if your UI is asynchronous, you're probably better off calling `Overlord` functions
//! so that you can know when they complete.  Generally they don't return anything of interest,
//! but will return an `Error` if that happens.  The result instead appears as a side-effect
//! either in GLOBALS data or in the database.
//!
//! # Storage
//!
//! Besides talking to the `Overlord`, the most common thing a front-end needs to do is interact
//! with the storage engine. In some cases, the `Overlord` has more complex code for doing this,
//! but in many cases, you can interact with `GLOBALS.storage` directly.

pub mod bookmarks;
pub use bookmarks::BookmarkList;

/// Defines messages sent to the overlord
pub mod comms;

mod delegation;
pub use delegation::Delegation;

mod dm_channel;
pub use dm_channel::{DmChannel, DmChannelData};

// direct quick-temporary communication with relays, without overlord/minion involvement
pub mod direct;

mod error;
pub use error::{Error, ErrorKind};

mod feed;
pub use feed::{Feed, FeedKind};

mod fetcher;
pub use fetcher::Fetcher;

mod filter;

mod globals;
pub use globals::{Globals, GLOBALS};

mod gossip_identity;
pub use gossip_identity::GossipIdentity;

mod media;
pub use media::Media;

mod misc;
pub use misc::{Freshness, Private, ZapState};

/// Rendering various names of users
pub mod names;

/// nip05 handling
pub mod nip05;

#[allow(dead_code)]
pub mod nip46;
pub use nip46::{Nip46Server, Nip46UnconnectedServer};

mod overlord;
pub use overlord::Overlord;

mod pending;
pub use pending::Pending;
pub use pending::PendingItem;

mod people;
pub use people::{hash_person_list_event, People, Person, PersonList, PersonListMetadata};

mod person_relay;
pub use person_relay::PersonRelay;

mod post;

/// Processing incoming events
pub mod process;

mod profile;
pub use profile::Profile;

mod relationship;

pub mod relay;
pub use relay::Relay;

pub mod relay_picker;
pub use relay_picker::RelayPicker;

mod seeker;
pub use seeker::Seeker;

mod status;
pub use status::StatusQueue;

mod storage;
pub use storage::types::*;
pub use storage::{PersonTable, Storage, Table};

mod tasks;

#[macro_use]
extern crate lazy_static;

/// The USER_AGENT string for gossip that it (may) use when fetching HTTP resources and
/// when connecting to relays
pub static USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

use nostr_types::EventKind;
use std::ops::DerefMut;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RunState {
    Initializing = 0,
    Offline = 1,
    Online = 2,
    ShuttingDown = 255,
}

impl RunState {
    #[inline]
    pub fn going_online(&self) -> bool {
        matches!(*self, RunState::Initializing | RunState::Online)
    }

    #[inline]
    pub fn going_offline(&self) -> bool {
        !self.going_online()
    }
}

impl std::convert::TryFrom<u8> for RunState {
    type Error = ();
    fn try_from(i: u8) -> Result<Self, Self::Error> {
        match i {
            x if x == RunState::Initializing as u8 => Ok(RunState::Initializing),
            x if x == RunState::Offline as u8 => Ok(RunState::Offline),
            x if x == RunState::Online as u8 => Ok(RunState::Online),
            x if x == RunState::ShuttingDown as u8 => Ok(RunState::ShuttingDown),
            _ => Err(()),
        }
    }
}

/// Initialize gossip-lib
pub fn init(rapid: bool) -> Result<(), Error> {
    use std::sync::atomic::Ordering;

    // Initialize storage
    GLOBALS.storage.init(rapid)?;

    // Load signer from settings
    GLOBALS.identity.load()?;

    // Load delegation tag
    GLOBALS.delegation.load()?;

    // If we have a key but have not unlocked it
    if GLOBALS.identity.has_private_key() && !GLOBALS.identity.is_unlocked() {
        // If we need to rebuild relationships
        if GLOBALS.storage.get_flag_rebuild_relationships_needed()
            || GLOBALS.storage.get_flag_rebuild_indexes_needed()
        {
            GLOBALS.wait_for_login.store(true, Ordering::Relaxed);
            GLOBALS
                .wait_for_data_migration
                .store(true, Ordering::Relaxed);
        } else if GLOBALS.storage.read_setting_login_at_startup() {
            GLOBALS.wait_for_login.store(true, Ordering::Relaxed);
        }
    }

    // Populate global bookmarks
    if let Some(pubkey) = GLOBALS.identity.public_key() {
        if let Some(event) =
            GLOBALS
                .storage
                .get_replaceable_event(EventKind::BookmarkList, pubkey, "")?
        {
            *GLOBALS.bookmarks.write() = BookmarkList::from_event(&event)?;
            GLOBALS.recompute_current_bookmarks.notify_one();
        }
    }

    Ok(())
}

/// Run gossip-lib as an async
pub async fn run() {
    // Runstate watcher
    tokio::task::spawn(async {
        let mut read_runstate = GLOBALS.read_runstate.clone();
        read_runstate.mark_unchanged();

        let mut last_runstate = *read_runstate.borrow();
        loop {
            // Wait for a change
            let _ = read_runstate.changed().await;

            // Verify it is actually a change, not set to the thing it already was set to
            if *read_runstate.borrow() != last_runstate {
                last_runstate = *read_runstate.borrow();

                tracing::info!("RunState changed to {:?}", last_runstate);

                // If we just went online, start all the tasks that come along with that
                // state transition
                if last_runstate == RunState::Online {
                    tracing::info!("Starting up online systems...");

                    // Start long-lived subscriptions
                    // (this also does a relay_picker init)
                    let _ = GLOBALS
                        .to_overlord
                        .send(crate::comms::ToOverlordMessage::StartLongLivedSubscriptions);
                }
            }
        }
    });

    // Steal `tmp_overlord_receiver` from the GLOBALS to give to a new Overlord
    let overlord_receiver = {
        let mut mutex_option = GLOBALS.tmp_overlord_receiver.lock().await;
        mutex_option.deref_mut().take()
    }
    .unwrap();

    // Run the overlord
    let mut overlord = Overlord::new(overlord_receiver);
    overlord.run().await;

    // Sync storage
    if let Err(e) = GLOBALS.storage.sync() {
        tracing::error!("{}", e);
    } else {
        tracing::info!("LMDB synced.");
    }

    tracing::error!("If gossip fails to exit at this point, you can safely kill the process.");
}
