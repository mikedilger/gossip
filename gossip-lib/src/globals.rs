use crate::comms::{RelayJob, ToMinionMessage, ToOverlordMessage};
use crate::error::Error;
use crate::feed::Feed;
use crate::fetcher::Fetcher;
use crate::gossip_identity::GossipIdentity;
use crate::media::Media;
use crate::misc::ZapState;
use crate::pending::Pending;
use crate::people::{People, Person};
use crate::relay::Relay;
use crate::relay_picker_hooks::Hooks;
use crate::seeker::Seeker;
use crate::status::StatusQueue;
use crate::storage::Storage;
use crate::RunState;
use dashmap::DashMap;
use gossip_relay_picker::RelayPicker;
use nostr_types::{Event, Id, Profile, PublicKey, RelayUrl, RelayUsage};
use parking_lot::RwLock as PRwLock;
use regex::Regex;
use rhai::{Engine, AST};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize};
use tokio::sync::watch::Receiver as WatchReceiver;
use tokio::sync::watch::Sender as WatchSender;
use tokio::sync::{broadcast, mpsc, Mutex, Notify, RwLock};

/// Global data shared between threads. Access via the static ref `GLOBALS`.
pub struct Globals {
    /// This is a broadcast channel. All Minions should listen on it.
    /// To create a receiver, just run .subscribe() on it.
    pub(crate) to_minions: broadcast::Sender<ToMinionMessage>,

    /// This is a mpsc channel. The Overlord listens on it.
    /// To create a sender, just clone() it.
    pub to_overlord: mpsc::UnboundedSender<ToOverlordMessage>,

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

    /// The relays not connected to, and which we will not connect to again until some
    /// time passes, but which we still have jobs for
    pub penalty_box_relays: DashMap<RelayUrl, Vec<RelayJob>>,

    /// The relay picker, used to pick the next relay
    pub relay_picker: RelayPicker<Hooks>,

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
    pub failed_avatars: RwLock<HashSet<PublicKey>>,

    pub pixels_per_point_times_100: AtomicU32,

    /// UI status messages
    pub status_queue: PRwLock<StatusQueue>,

    /// How many data bytes have been read from the network, not counting overhead
    pub bytes_read: AtomicUsize,

    /// How many subscriptions are open and not yet at EOSE
    pub open_subscriptions: AtomicUsize,

    /// Media loading
    pub media: Media,

    /// Search results
    pub events_being_searched_for: PRwLock<Vec<Id>>, // being searched for
    //pub event_addrs_being_searched_for: PRwLock<Vec<EventAddr>>, // being searched for
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

    /// Current zap data, for UI
    pub current_zap: PRwLock<ZapState>,

    /// Hashtag regex
    pub hashtag_regex: Regex,

    /// Tagging regex
    pub tagging_regex: Regex,

    /// LMDB storage
    pub storage: Storage,

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
}

lazy_static! {
    /// A static reference to global data shared between threads.
    pub static ref GLOBALS: Globals = {

        // Setup a communications channel from the Overlord to the Minions.
        let (to_minions, _) = broadcast::channel(512);

        // Setup a communications channel from the Minions to the Overlord.
        let (to_overlord, tmp_overlord_receiver) = mpsc::unbounded_channel();

        // Setup a watch channel for going offline state change
        // We start in the Offline state
        let (write_runstate, read_runstate) = tokio::sync::watch::channel(RunState::Initializing);

        let storage = match Storage::new() {
            Ok(s) => s,
            Err(e) => panic!("{e}")
        };

        let filter_engine = Engine::new();
        let filter = crate::filter::load_script(&filter_engine);

        Globals {
            to_minions,
            to_overlord,
            write_runstate,
            read_runstate,
            tmp_overlord_receiver: Mutex::new(Some(tmp_overlord_receiver)),
            people: People::new(),
            connected_relays: DashMap::new(),
            penalty_box_relays: DashMap::new(),
            relay_picker: Default::default(),
            identity: GossipIdentity::default(),
            dismissed: RwLock::new(Vec::new()),
            feed: Feed::new(),
            fetcher: Fetcher::new(),
            seeker: Seeker::new(),
            failed_avatars: RwLock::new(HashSet::new()),
            pixels_per_point_times_100: AtomicU32::new(139), // 100 dpi, 1/72th inch => 1.38888
            status_queue: PRwLock::new(StatusQueue::new(
                "Welcome to Gossip. Status messages will appear here. Click them to dismiss them.".to_owned()
            )),
            bytes_read: AtomicUsize::new(0),
            open_subscriptions: AtomicUsize::new(0),
            media: Media::new(),
            events_being_searched_for: PRwLock::new(Vec::new()),
            //event_addrs_being_searched_for: PRwLock::new(Vec::new()),
            people_search_results: PRwLock::new(Vec::new()),
            note_search_results: PRwLock::new(Vec::new()),
            ui_notes_to_invalidate: PRwLock::new(Vec::new()),
            ui_people_to_invalidate: PRwLock::new(Vec::new()),
            ui_invalidate_all: AtomicBool::new(false),
            current_zap: PRwLock::new(ZapState::None),
            hashtag_regex: Regex::new(r"(?:^|\W)(#[\w\p{Extended_Pictographic}]+)(?:$|\W)").unwrap(),
            tagging_regex: Regex::new(r"(?:^|\s+)@([\w\p{Extended_Pictographic}]+)(?:$|\W)").unwrap(),
            storage,
            events_processed: AtomicU32::new(0),
            filter_engine,
            filter,
            wait_for_login: AtomicBool::new(false),
            wait_for_login_notify: Notify::new(),
            wait_for_data_migration: AtomicBool::new(false),
            advertise_jobs_remaining: AtomicUsize::new(0),
            pending: Pending::new(),
            loading_more: AtomicUsize::new(0),
        }
    };
}

impl Globals {
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

    // Which relays should an event be posted to (that it hasn't already been
    // seen on)?
    pub fn relays_for_event(event: &Event) -> Result<Vec<RelayUrl>, Error> {
        let num_relays_per_person = GLOBALS.storage.read_setting_num_relays_per_person();
        let mut relay_urls: Vec<RelayUrl> = Vec::new();

        // Get all of the relays that we write to
        let write_relay_urls: Vec<RelayUrl> = GLOBALS
            .storage
            .filter_relays(|r| r.has_usage_bits(Relay::WRITE) && r.rank != 0)?
            .iter()
            .map(|relay| relay.url.clone())
            .collect();
        relay_urls.extend(write_relay_urls);

        // Get 'read' relays for everybody tagged in the event.
        let mut tagged_pubkeys: Vec<PublicKey> = event
            .tags
            .iter()
            .filter_map(|t| {
                if let Ok((pubkey, _, _)) = t.parse_pubkey() {
                    Some(pubkey)
                } else {
                    None
                }
            })
            .collect();
        for pubkey in tagged_pubkeys.drain(..) {
            let best_relays: Vec<RelayUrl> = GLOBALS
                .storage
                .get_best_relays(pubkey, RelayUsage::Inbox)?
                .drain(..)
                .take(num_relays_per_person as usize + 1)
                .map(|(u, _)| u)
                .collect();
            relay_urls.extend(best_relays);
        }

        // Remove all the 'seen_on' relays for this event
        let seen_on: Vec<RelayUrl> = GLOBALS
            .storage
            .get_event_seen_on_relay(event.id)?
            .iter()
            .map(|(url, _time)| url.to_owned())
            .collect();
        relay_urls.retain(|r| !seen_on.contains(r));

        relay_urls.sort();
        relay_urls.dedup();

        Ok(relay_urls)
    }
}
