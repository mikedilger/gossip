use crate::comms::{RelayJob, ToMinionMessage, ToOverlordMessage};
use crate::delegation::Delegation;
use crate::feed::Feed;
use crate::fetcher::Fetcher;
use crate::media::Media;
use crate::people::{People, Person};
use crate::relay::Relay;
use crate::relay_picker_hooks::Hooks;
use crate::settings::Settings;
use crate::signer::Signer;
use crate::status::StatusQueue;
use crate::storage::Storage;
use dashmap::DashMap;
use gossip_relay_picker::RelayPicker;
use nostr_types::{Event, Id, PayRequestData, Profile, PublicKey, RelayUrl, UncheckedUrl};
use parking_lot::RwLock as PRwLock;
use regex::Regex;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize};
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};

#[derive(Debug, Clone)]
pub enum ZapState {
    None,
    CheckingLnurl(Id, PublicKey, UncheckedUrl),
    SeekingAmount(Id, PublicKey, PayRequestData, UncheckedUrl),
    LoadingInvoice(Id, PublicKey),
    ReadyToPay(Id, String), // String is the Zap Invoice as a string, to be shown as a QR code
}

/// Only one of these is ever created, via lazy_static!, and represents
/// global state for the rust application
pub struct Globals {
    /// Is this the first run?
    pub first_run: AtomicBool,

    /// This is a broadcast channel. All Minions should listen on it.
    /// To create a receiver, just run .subscribe() on it.
    pub to_minions: broadcast::Sender<ToMinionMessage>,

    /// This is a mpsc channel. The Overlord listens on it.
    /// To create a sender, just clone() it.
    pub to_overlord: mpsc::UnboundedSender<ToOverlordMessage>,

    /// This is ephemeral. It is filled during lazy_static initialization,
    /// and stolen away when the Overlord is created.
    pub tmp_overlord_receiver: Mutex<Option<mpsc::UnboundedReceiver<ToOverlordMessage>>>,

    /// All nostr people records currently loaded into memory, keyed by pubkey
    pub people: People,

    /// The relays currently connected to
    pub connected_relays: DashMap<RelayUrl, Vec<RelayJob>>,

    /// The relay picker, used to pick the next relay
    pub relay_picker: RelayPicker<Hooks>,

    /// Whether or not we are shutting down. For the UI (minions will be signaled and
    /// waited for by the overlord)
    pub shutting_down: AtomicBool,

    /// Settings
    pub settings: PRwLock<Settings>,

    /// Signer
    pub signer: Signer,

    /// Dismissed Events
    pub dismissed: RwLock<Vec<Id>>,

    /// Feed
    pub feed: Feed,

    /// Fetcher
    pub fetcher: Fetcher,

    /// Failed Avatars
    /// If in this map, the avatar failed to load or process and is unrecoverable
    /// (but we will take them out and try again if new metadata flows in)
    pub failed_avatars: RwLock<HashSet<PublicKey>>,

    pub pixels_per_point_times_100: AtomicU32,

    /// UI status messages
    pub status_queue: PRwLock<StatusQueue>,

    /// How many data bytes have been read from the network, not counting overhead
    pub bytes_read: AtomicUsize,

    /// How many subscriptions are open and not yet at EOSE
    pub open_subscriptions: AtomicUsize,

    /// Delegation handling
    pub delegation: Delegation,

    /// Media loading
    pub media: Media,

    /// Search results
    pub people_search_results: PRwLock<Vec<Person>>,
    pub note_search_results: PRwLock<Vec<Event>>,

    /// UI note cache invalidation per note
    // when we update an augment (deletion/reaction/zap) the UI must recompute
    pub ui_notes_to_invalidate: PRwLock<Vec<Id>>,

    /// UI note cache invalidation per person
    // when we update a Person, the UI must recompute all notes by them
    pub ui_people_to_invalidate: PRwLock<Vec<PublicKey>>,

    /// Current zap data, for UI
    pub current_zap: PRwLock<ZapState>,

    /// Hashtag regex
    pub hashtag_regex: Regex,

    /// LMDB storage
    pub storage: Storage,

    /// Events Processed
    pub events_processed: AtomicU32,
}

lazy_static! {
    pub static ref GLOBALS: Globals = {

        // Setup a communications channel from the Overlord to the Minions.
        let (to_minions, _) = broadcast::channel(512);

        // Setup a communications channel from the Minions to the Overlord.
        let (to_overlord, tmp_overlord_receiver) = mpsc::unbounded_channel();

        let storage = match Storage::new() {
            Ok(s) => s,
            Err(e) => panic!("{e}")
        };

        Globals {
            first_run: AtomicBool::new(false),
            to_minions,
            to_overlord,
            tmp_overlord_receiver: Mutex::new(Some(tmp_overlord_receiver)),
            people: People::new(),
            connected_relays: DashMap::new(),
            relay_picker: Default::default(),
            shutting_down: AtomicBool::new(false),
            settings: PRwLock::new(Settings::default()),
            signer: Signer::default(),
            dismissed: RwLock::new(Vec::new()),
            feed: Feed::new(),
            fetcher: Fetcher::new(),
            failed_avatars: RwLock::new(HashSet::new()),
            pixels_per_point_times_100: AtomicU32::new(139), // 100 dpi, 1/72th inch => 1.38888
            status_queue: PRwLock::new(StatusQueue::new(
                "Welcome to Gossip. Status messages will appear here. Click them to dismiss them.".to_owned()
            )),
            bytes_read: AtomicUsize::new(0),
            open_subscriptions: AtomicUsize::new(0),
            delegation: Delegation::default(),
            media: Media::new(),
            people_search_results: PRwLock::new(Vec::new()),
            note_search_results: PRwLock::new(Vec::new()),
            ui_notes_to_invalidate: PRwLock::new(Vec::new()),
            ui_people_to_invalidate: PRwLock::new(Vec::new()),
            current_zap: PRwLock::new(ZapState::None),
            hashtag_regex: Regex::new(r"(?:^|\W)(#[\w\p{Extended_Pictographic}]+)(?:$|\W)").unwrap(),
            storage,
            events_processed: AtomicU32::new(0),
        }
    };
}

impl Globals {
    pub fn get_your_nprofile() -> Option<Profile> {
        let public_key = match GLOBALS.signer.public_key() {
            Some(pk) => pk,
            None => return None,
        };

        let mut profile = Profile {
            pubkey: public_key,
            relays: Vec::new(),
        };

        match GLOBALS
            .storage
            .filter_relays(|ri| ri.has_usage_bits(Relay::OUTBOX))
        {
            Err(e) => {
                tracing::error!("{}", e);
                return None;
            }
            Ok(relays) => {
                for relay in relays {
                    profile.relays.push(relay.url.to_unchecked_url());
                }
            }
        }

        Some(profile)
    }
}
