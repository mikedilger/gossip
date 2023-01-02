use crate::comms::BusMessage;
use crate::db::{DbEvent, DbRelay};
use crate::error::Error;
use crate::feed::Feed;
use crate::fetcher::Fetcher;
use crate::people::People;
use crate::relationship::Relationship;
use crate::settings::Settings;
use crate::signer::Signer;
use nostr_types::{Event, Id, IdHex, PublicKeyHex, Unixtime, Url};
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};

/// Only one of these is ever created, via lazy_static!, and represents
/// global state for the rust application
pub struct Globals {
    /// This is our connection to SQLite. Only one thread at a time.
    pub db: Mutex<Option<Connection>>,

    /// This is a broadcast channel. All Minions should listen on it.
    /// To create a receiver, just run .subscribe() on it.
    pub to_minions: broadcast::Sender<BusMessage>,

    /// This is a mpsc channel. The Overlord listens on it.
    /// To create a sender, just clone() it.
    pub to_overlord: mpsc::UnboundedSender<BusMessage>,

    /// This is ephemeral. It is filled during lazy_static initialization,
    /// and stolen away when the Overlord is created.
    pub tmp_overlord_receiver: Mutex<Option<mpsc::UnboundedReceiver<BusMessage>>>,

    /// All nostr events, keyed by the event Id
    pub events: RwLock<HashMap<Id, Event>>,

    /// Events coming in from relays that are not processed yet
    pub incoming_events: RwLock<Vec<(Event, Url)>>,

    /// All relationships between events
    pub relationships: RwLock<HashMap<Id, Vec<(Id, Relationship)>>>,

    /// The date of the latest reply. Only reply relationships count, not reactions,
    /// deletions, or quotes
    pub last_reply: RwLock<HashMap<Id, Unixtime>>,

    /// Desired events, referred to by others, with possible URLs where we can
    /// get them.  We may already have these, but if not we should ask for them.
    pub desired_events: RwLock<HashMap<Id, Vec<Url>>>,

    /// All nostr people records currently loaded into memory, keyed by pubkey
    pub people: RwLock<People>,

    /// All nostr relay records we have
    pub relays: RwLock<HashMap<Url, DbRelay>>,

    /// Whether or not we are shutting down. For the UI (minions will be signaled and
    /// waited for by the overlord)
    pub shutting_down: AtomicBool,

    /// Settings
    pub settings: RwLock<Settings>,

    /// Signer
    pub signer: RwLock<Signer>,

    /// Dismissed Events
    pub dismissed: RwLock<Vec<Id>>,

    /// Event is new
    pub event_is_new: RwLock<Vec<Id>>,

    /// Feed
    pub feed: Feed,

    /// Fetcher
    pub fetcher: Fetcher,

    /// Failed Avatar Fetches
    pub failed_avatars: RwLock<HashSet<PublicKeyHex>>,
}

lazy_static! {
    pub static ref GLOBALS: Globals = {

        // Setup a communications channel from the Overlord to the Minions.
        let (to_minions, _) = broadcast::channel(256);

        // Setup a communications channel from the Minions to the Overlord.
        let (to_overlord, tmp_overlord_receiver) = mpsc::unbounded_channel();

        Globals {
            db: Mutex::new(None),
            to_minions,
            to_overlord,
            tmp_overlord_receiver: Mutex::new(Some(tmp_overlord_receiver)),
            events: RwLock::new(HashMap::new()),
            incoming_events: RwLock::new(Vec::new()),
            relationships: RwLock::new(HashMap::new()),
            last_reply: RwLock::new(HashMap::new()),
            desired_events: RwLock::new(HashMap::new()),
            people: RwLock::new(People::new()),
            relays: RwLock::new(HashMap::new()),
            shutting_down: AtomicBool::new(false),
            settings: RwLock::new(Settings::default()),
            signer: RwLock::new(Signer::default()),
            dismissed: RwLock::new(Vec::new()),
            event_is_new: RwLock::new(Vec::new()),
            feed: Feed::new(),
            fetcher: Fetcher::new(),
            failed_avatars: RwLock::new(HashSet::new()),
        }
    };
}

impl Globals {
    pub async fn store_desired_event(id: Id, url: Option<Url>) {
        let mut desired_events = GLOBALS.desired_events.write().await;
        desired_events
            .entry(id)
            .and_modify(|urls| {
                if let Some(ref u) = url {
                    let n = Url::new(u);
                    if n.is_valid_relay_url() {
                        urls.push(n);
                    }
                }
            })
            .or_insert_with(|| if let Some(u) = url { vec![u] } else { vec![] });
    }

    pub fn trim_desired_events_sync() {
        // danger - two locks could lead to deadlock, check other code locking these
        // don't change the order, or else change it everywhere
        let mut desired_events = GLOBALS.desired_events.blocking_write();
        let events = GLOBALS.events.blocking_read();
        desired_events.retain(|&id, _| !events.contains_key(&id));
    }

    pub async fn trim_desired_events() {
        // danger - two locks could lead to deadlock, check other code locking these
        // don't change the order, or else change it everywhere
        let mut desired_events = GLOBALS.desired_events.write().await;
        let events = GLOBALS.events.read().await;
        desired_events.retain(|&id, _| !events.contains_key(&id));
    }

    async fn get_desired_events_prelude() -> Result<(), Error> {
        Self::trim_desired_events().await;

        // Load from database
        {
            let ids: Vec<IdHex> = GLOBALS
                .desired_events
                .read()
                .await
                .iter()
                .map(|(id, _)| Into::<IdHex>::into(*id))
                .collect();
            let db_events = DbEvent::fetch_by_ids(ids).await?;
            let mut events: Vec<Event> = Vec::with_capacity(db_events.len());
            for dbevent in db_events.iter() {
                let e = serde_json::from_str(&dbevent.raw)?;
                events.push(e);
            }
            let mut count = 0;
            for event in events.iter() {
                count += 1;
                crate::process::process_new_event(event, false, None).await?;
            }
            tracing::info!("Loaded {} desired events from the database", count);
        }

        Self::trim_desired_events().await; // again

        Ok(())
    }

    pub async fn get_desired_events() -> Result<(HashMap<Url, Vec<Id>>, Vec<Id>), Error> {
        Globals::get_desired_events_prelude().await?;

        let desired_events = GLOBALS.desired_events.read().await;
        let mut output: HashMap<Url, Vec<Id>> = HashMap::new();
        let mut orphans: Vec<Id> = Vec::new();
        for (id, vec_url) in desired_events.iter() {
            if vec_url.is_empty() {
                orphans.push(*id);
            } else {
                for url in vec_url.iter() {
                    output
                        .entry(url.to_owned())
                        .and_modify(|vec| vec.push(*id))
                        .or_insert_with(|| vec![*id]);
                }
            }
        }

        Ok((output, orphans))
    }

    /*
    pub async fn get_desired_events_for_url(url: Url) -> Result<Vec<Id>, Error> {
        Globals::get_desired_events_prelude().await?;

        let desired_events = GLOBALS.desired_events.read().await;
        let mut output: Vec<Id> = Vec::new();
        for (id, vec_url) in desired_events.iter() {
            if vec_url.is_empty() || vec_url.contains(&url) {
                output.push(*id);
            }
        }

        Ok(output)
    }
     */

    pub async fn add_relationship(id: Id, related: Id, relationship: Relationship) {
        let r = (related, relationship);
        let mut relationships = GLOBALS.relationships.write().await;
        relationships
            .entry(id)
            .and_modify(|vec| {
                if !vec.contains(&r) {
                    vec.push(r.clone());
                }
            })
            .or_insert_with(|| vec![r]);
    }

    pub async fn update_last_reply(id: Id, time: Unixtime) {
        let mut last_reply = GLOBALS.last_reply.write().await;
        last_reply
            .entry(id)
            .and_modify(|lasttime| {
                if time > *lasttime {
                    *lasttime = time;
                }
            })
            .or_insert_with(|| time);
    }

    pub fn get_replies_sync(id: Id) -> Vec<Id> {
        let mut output: Vec<Id> = Vec::new();
        if let Some(vec) = GLOBALS.relationships.blocking_read().get(&id) {
            for (id, relationship) in vec.iter() {
                if *relationship == Relationship::Reply {
                    output.push(*id);
                }
            }
        }

        output
    }

    // FIXME - this allows people to react many times to the same event, and
    //         it counts them all!
    pub fn get_reactions_sync(id: Id) -> Vec<(char, usize)> {
        let mut output: HashMap<char, usize> = HashMap::new();

        if let Some(relationships) = GLOBALS.relationships.blocking_read().get(&id).cloned() {
            for (_id, relationship) in relationships.iter() {
                if let Relationship::Reaction(reaction) = relationship {
                    if let Some(ch) = reaction.chars().next() {
                        output
                            .entry(ch)
                            .and_modify(|count| *count += 1)
                            .or_insert_with(|| 1);
                    } else {
                        output
                            .entry('+') // if empty, presumed to be an upvote
                            .and_modify(|count| *count += 1)
                            .or_insert_with(|| 1);
                    }
                }
            }
        }

        let mut v: Vec<(char, usize)> = output.iter().map(|(c, u)| (*c, *u)).collect();
        v.sort();
        v
    }
}
