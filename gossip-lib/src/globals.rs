use crate::bookmarks::BookmarkList;
use crate::comms::{RelayJob, ToMinionMessage, ToOverlordMessage};
use crate::delegation::Delegation;
use crate::error::Error;
use crate::feed::Feed;
use crate::fetcher::Fetcher;
use crate::gossip_identity::GossipIdentity;
use crate::media::Media;
use crate::minion::MinionExitReason;
use crate::misc::ZapState;
use crate::pending::Pending;
use crate::people::{People, Person};
use crate::relay::Relay;
use crate::relay_picker::RelayPicker;
use crate::seeker::Seeker;
use crate::status::StatusQueue;
use crate::storage::Storage;
use crate::RunState;
use dashmap::DashMap;
use nostr_types::{Event, Id, Profile, PublicKey, RelayUrl};
use parking_lot::RwLock as PRwLock;
use regex::Regex;
use rhai::{Engine, AST};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize};
use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;
use tokio::sync::watch::Receiver as WatchReceiver;
use tokio::sync::watch::Sender as WatchSender;
use tokio::sync::{broadcast, mpsc, Mutex, Notify, RwLock};

/// Global data shared between threads. Access via the static ref `GLOBALS`.
pub struct Globals {
    /// The tokio runtime
    pub runtime: Arc<Runtime>,

    /// This is a broadcast channel. All Minions should listen on it.
    /// To create a receiver, just run .subscribe() on it.
    pub(crate) to_minions: broadcast::Sender<ToMinionMessage>,

    /// This is a mpsc channel. The Overlord listens on it.
    /// To create a sender, just clone() it.
    pub to_overlord: mpsc::UnboundedSender<ToOverlordMessage>,

    /// Current minion tasks
    pub minions: RwLock<tokio::task::JoinSet<Result<MinionExitReason, Error>>>,

    /// Map from minion task id to relay url
    pub minions_task_url: DashMap<tokio::task::Id, RelayUrl>,

    /// This is a watch channel for making changes to the RunState.
    pub write_runstate: WatchSender<RunState>,

    /// This is a watch channel for watching for changes to the RunState.
    ///
    /// Synchronous code can `borrow()` and dereference to see the current Runstate.
    ///
    /// Asynchronous code should `clone()` and `.await` on the clone (please do not
    /// await on this global source copy, because if two or more bits of code to that,
    /// only one of them will get awoken).
    pub read_runstate: WatchReceiver<RunState>,

    /// This is ephemeral. It is filled during lazy_static initialization,
    /// and needs to be stolen away and given to the Overlord when the Overlord
    /// is created.
    pub tmp_overlord_receiver: Mutex<Option<mpsc::UnboundedReceiver<ToOverlordMessage>>>,

    /// All nostr people records currently loaded into memory, keyed by pubkey
    pub people: People,

    /// The relays currently connected to. It tracks the jobs that relay is assigned.
    /// As the minion completes jobs, code modifies this jobset, removing those completed
    /// jobs.
    pub connected_relays: DashMap<RelayUrl, Vec<RelayJob>>,

    /// The relay picker, used to pick the next relay
    pub relay_picker: RelayPicker,

    /// Wrapped Identity wrapping a Signer
    pub identity: GossipIdentity,

    /// Dismissed Events
    pub dismissed: RwLock<Vec<Id>>,

    /// Feed
    pub feed: Feed,

    /// Fetcher
    pub fetcher: Fetcher,

    /// Seeker
    pub seeker: Seeker,

    /// Failed Avatars
    /// If in this map, the avatar failed to load or process and is unrecoverable
    /// (but we will take them out and try again if new metadata flows in)
    pub failed_avatars: PRwLock<HashSet<PublicKey>>,

    pub pixels_per_point_times_100: AtomicU32,

    /// UI status messages
    pub status_queue: PRwLock<StatusQueue>,

    /// How many data bytes have been read from the network, not counting overhead
    pub bytes_read: AtomicUsize,

    /// How many subscriptions are open and not yet at EOSE
    pub open_subscriptions: AtomicUsize,

    /// How many unread direct messages
    pub unread_dms: AtomicUsize,

    /// Delegation handling
    pub delegation: Delegation,

    /// Media loading
    pub media: Media,

    /// Search results
    pub events_being_searched_for: PRwLock<Vec<Id>>, // being searched for
    //pub naddrs_being_searched_for: PRwLock<Vec<NAddr>>, // being searched for
    pub people_search_results: PRwLock<Vec<Person>>,
    pub note_search_results: PRwLock<Vec<Event>>,

    /// UI note cache invalidation per note
    // when we update an augment (deletion/reaction/zap) the UI must recompute
    pub ui_notes_to_invalidate: PRwLock<Vec<Id>>,

    /// UI note cache invalidation per person
    // when we update a Person, the UI must recompute all notes by them
    pub ui_people_to_invalidate: PRwLock<Vec<PublicKey>>,

    /// UI invalidate all
    pub ui_invalidate_all: AtomicBool,

    /// Max image side length that can be rendered
    pub max_image_side: AtomicUsize,

    /// Current zap data, for UI
    pub current_zap: PRwLock<ZapState>,

    /// Hashtag regex
    pub hashtag_regex: Regex,

    /// Tagging regex
    pub tagging_regex: Regex,

    /// LMDB storage
    pub storage: OnceLock<Storage>,

    /// Events Processed
    pub events_processed: AtomicU32,

    /// Filter
    pub(crate) filter_engine: Engine,
    pub(crate) filter: Option<AST>,

    // Wait for login
    pub wait_for_login: AtomicBool,
    pub wait_for_login_notify: Notify,

    // Wait for data migration
    pub wait_for_data_migration: AtomicBool,

    // Active advertise jobs
    pub advertise_jobs_remaining: AtomicUsize,

    /// Pending actions
    pub pending: Pending,

    /// Loading more - how many relays are still loading a chunk of events.
    pub loading_more: AtomicUsize,

    /// Bookmarks
    pub bookmarks: RwLock<BookmarkList>,

    /// Current bookmarks, resolved into a Vec<Id> (updated by tasks)
    pub current_bookmarks: PRwLock<Vec<Id>>,
    pub recompute_current_bookmarks: Arc<Notify>,
}

lazy_static! {
    /// A static reference to global data shared between threads.
    pub static ref GLOBALS: Globals = {
        let runtime = tokio::runtime::Runtime::new().unwrap();

        // Setup a communications channel from the Overlord to the Minions.
        let (to_minions, _) = broadcast::channel(2048);

        // Setup a communications channel from the Minions to the Overlord.
        let (to_overlord, tmp_overlord_receiver) = mpsc::unbounded_channel();

        // Setup a watch channel for going offline state change
        // We start in the Offline state
        let (write_runstate, read_runstate) = tokio::sync::watch::channel(RunState::Initializing);

        let filter_engine = Engine::new();
        let filter = crate::filter::load_script(&filter_engine);

        Globals {
            runtime: Arc::new(runtime),
            to_minions,
            to_overlord,
            minions: RwLock::new(tokio::task::JoinSet::new()),
            minions_task_url: DashMap::new(),
            write_runstate,
            read_runstate,
            tmp_overlord_receiver: Mutex::new(Some(tmp_overlord_receiver)),
            people: People::new(),
            connected_relays: DashMap::new(),
            relay_picker: Default::default(),
            identity: GossipIdentity::default(),
            dismissed: RwLock::new(Vec::new()),
            feed: Feed::new(),
            fetcher: Fetcher::new(),
            seeker: Seeker::new(),
            failed_avatars: PRwLock::new(HashSet::new()),
            pixels_per_point_times_100: AtomicU32::new(139), // 100 dpi, 1/72th inch => 1.38888
            status_queue: PRwLock::new(StatusQueue::new(
                "Welcome to Gossip. Status messages will appear here. Click them to dismiss them.".to_owned()
            )),
            bytes_read: AtomicUsize::new(0),
            open_subscriptions: AtomicUsize::new(0),
            unread_dms: AtomicUsize::new(0),
            delegation: Delegation::default(),
            media: Media::new(),
            events_being_searched_for: PRwLock::new(Vec::new()),
            //naddrs_being_searched_for: PRwLock::new(Vec::new()),
            people_search_results: PRwLock::new(Vec::new()),
            note_search_results: PRwLock::new(Vec::new()),
            ui_notes_to_invalidate: PRwLock::new(Vec::new()),
            ui_people_to_invalidate: PRwLock::new(Vec::new()),
            ui_invalidate_all: AtomicBool::new(false),
            max_image_side: AtomicUsize::new(2048),
            current_zap: PRwLock::new(ZapState::None),
            hashtag_regex: Regex::new(r"(?:^|\W)(#[\w\p{Extended_Pictographic}]+)(?:$|\W)").unwrap(),
            tagging_regex: Regex::new(r"(?:^|\s+)@([\w\p{Extended_Pictographic}]+)(?:$|\W)").unwrap(),
            storage: OnceLock::new(),
            events_processed: AtomicU32::new(0),
            filter_engine,
            filter,
            wait_for_login: AtomicBool::new(false),
            wait_for_login_notify: Notify::new(),
            wait_for_data_migration: AtomicBool::new(false),
            advertise_jobs_remaining: AtomicUsize::new(0),
            pending: Pending::new(),
            loading_more: AtomicUsize::new(0),
            bookmarks: RwLock::new(BookmarkList::empty()),
            current_bookmarks: PRwLock::new(Vec::new()),
            recompute_current_bookmarks: Arc::new(Notify::new()),
        }
    };
}

impl Globals {
    pub fn db(&self) -> &Storage {
        match self.storage.get() {
            Some(s) => s,
            None => panic!("Storage call before initialization"),
        }
    }

    pub fn get_your_nprofile() -> Option<Profile> {
        let public_key = match GLOBALS.identity.public_key() {
            Some(pk) => pk,
            None => return None,
        };

        let mut profile = Profile {
            pubkey: public_key,
            relays: Vec::new(),
        };

        match GLOBALS
            .db()
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
