use crate::comms::BusMessage;
use crate::db::{DbPerson, DbPersonRelay, DbRelay};
use crate::relationship::Relationship;
use crate::settings::Settings;
use async_recursion::async_recursion;
use nostr_types::{Event, EventKind, Id, PublicKey, PublicKeyHex, Unixtime, Url};
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use tokio::sync::{broadcast, mpsc, Mutex};

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
    pub from_minions: Mutex<Option<mpsc::UnboundedReceiver<BusMessage>>>,

    /// All nostr events, keyed by the event Id
    pub events: Mutex<HashMap<Id, Event>>,

    /// All relationships between events
    pub relationships: Mutex<HashMap<Id, Vec<(Id, Relationship)>>>,

    /// The date of the latest reply. Only reply relationships count, not reactions,
    /// deletions, or quotes
    pub last_reply: Mutex<HashMap<Id, Unixtime>>,

    /// Desired events, referred to by others, with possible URLs where we can
    /// get them.  We may already have these, but if not we should ask for them.
    pub desired_events: Mutex<HashMap<Id, Vec<Url>>>,

    /// All nostr people records currently loaded into memory, keyed by pubkey
    pub people: Mutex<HashMap<PublicKey, DbPerson>>,

    /// All nostr relay records we have
    pub relays: Mutex<HashMap<Url, DbRelay>>,

    /// Whether or not we have a saved private key and need the password to unlock it
    pub need_password: AtomicBool,

    /// Whether or not we are shutting down. For the UI (minions will be signaled and
    /// waited for by the overlord)
    pub shutting_down: AtomicBool,

    /// Settings
    pub settings: Mutex<Settings>,
}

lazy_static! {
    pub static ref GLOBALS: Globals = {

        // Setup a communications channel from the Overlord to the Minions.
        let (to_minions, _) = broadcast::channel(16);

        // Setup a communications channel from the Minions to the Overlord.
        let (to_overlord, from_minions) = mpsc::unbounded_channel();

        Globals {
            db: Mutex::new(None),
            to_minions,
            to_overlord,
            from_minions: Mutex::new(Some(from_minions)),
            events: Mutex::new(HashMap::new()),
            relationships: Mutex::new(HashMap::new()),
            last_reply: Mutex::new(HashMap::new()),
            desired_events: Mutex::new(HashMap::new()),
            people: Mutex::new(HashMap::new()),
            relays: Mutex::new(HashMap::new()),
            need_password: AtomicBool::new(false),
            shutting_down: AtomicBool::new(false),
            settings: Mutex::new(Settings::default()),
        }
    };
}

impl Globals {
    pub fn blocking_get_feed(threaded: bool) -> Vec<Id> {
        let feed: Vec<Event> = GLOBALS
            .events
            .blocking_lock()
            .iter()
            .map(|(_, e)| e)
            .filter(|e| e.kind == EventKind::TextNote)
            .filter(|e| {
                if threaded {
                    e.replies_to().is_none()
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        Self::sort_feed(feed, threaded)
    }

    fn sort_feed(mut feed: Vec<Event>, threaded: bool) -> Vec<Id> {
        if threaded {
            feed.sort_unstable_by(|a, b| {
                let a_last = GLOBALS.last_reply.blocking_lock().get(&a.id).cloned();
                let b_last = GLOBALS.last_reply.blocking_lock().get(&b.id).cloned();
                let a_time = a_last.unwrap_or(a.created_at);
                let b_time = b_last.unwrap_or(b.created_at);
                b_time.cmp(&a_time)
            });
        } else {
            feed.sort_unstable_by(|a, b| b.created_at.cmp(&a.created_at));
        }

        feed.iter().map(|e| e.id).collect()
    }

    pub async fn store_desired_event(id: Id, url: Option<Url>) {
        let mut desired_events = GLOBALS.desired_events.lock().await;
        desired_events
            .entry(id)
            .and_modify(|urls| {
                if let Some(ref u) = url {
                    urls.push(u.to_owned());
                }
            })
            .or_insert_with(|| if let Some(u) = url { vec![u] } else { vec![] });
    }

    pub async fn add_relationship(id: Id, related: Id, relationship: Relationship) {
        let r = (related, relationship);
        let mut relationships = GLOBALS.relationships.lock().await;
        relationships
            .entry(id)
            .and_modify(|vec| {
                if !vec.contains(&r) {
                    vec.push(r.clone());
                }
            })
            .or_insert_with(|| vec![r]);
    }

    #[async_recursion]
    pub async fn update_last_reply(id: Id, time: Unixtime) {
        {
            let mut last_reply = GLOBALS.last_reply.lock().await;
            last_reply
                .entry(id)
                .and_modify(|lasttime| {
                    if time > *lasttime {
                        *lasttime = time;
                    }
                })
                .or_insert_with(|| time);
        } // drops lock

        // Recurse upwards
        if let Some(event) = GLOBALS.events.lock().await.get(&id).cloned() {
            if let Some((id, _maybe_url)) = event.replies_to() {
                Self::update_last_reply(id, event.created_at).await;
            }
        }
    }

    pub fn get_replies_sync(id: Id) -> Vec<Id> {
        let mut output: Vec<Id> = Vec::new();
        if let Some(vec) = GLOBALS.relationships.blocking_lock().get(&id) {
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
    pub fn get_reactions_sync(id: Id) -> HashMap<char, usize> {
        let mut output: HashMap<char, usize> = HashMap::new();

        if let Some(relationships) = GLOBALS.relationships.blocking_lock().get(&id).cloned() {
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

        output
    }
}

pub async fn followed_pubkeys() -> Vec<PublicKeyHex> {
    let people = GLOBALS.people.lock().await;
    people
        .iter()
        .map(|(_, p)| p)
        .filter(|p| p.followed == 1)
        .map(|p| p.pubkey.clone())
        .collect()
}

#[allow(dead_code)]
pub async fn follow_key_and_relay(pubkey: String, relay: String) -> Result<DbPerson, String> {
    let pubkeyhex = PublicKeyHex(pubkey.clone());

    // Create or update them
    let person = match DbPerson::fetch_one(pubkeyhex.clone())
        .await
        .map_err(|e| format!("{}", e))?
    {
        Some(mut person) => {
            person.followed = 1;
            DbPerson::update(person.clone())
                .await
                .map_err(|e| format!("{}", e))?;
            person
        }
        None => {
            let mut person = DbPerson::new(pubkeyhex.clone());
            person.followed = 1;
            DbPerson::insert(person.clone())
                .await
                .map_err(|e| format!("{}", e))?;
            person
        }
    };

    // Insert (or ignore) this relay
    DbRelay::insert(DbRelay::new(relay.clone()))
        .await
        .map_err(|e| format!("{}", e))?;

    // Insert (or ignore) this person's relay
    DbPersonRelay::insert(DbPersonRelay {
        person: pubkey,
        relay,
        ..Default::default()
    })
    .await
    .map_err(|e| format!("{}", e))?;

    // Tell the overlord to update the  minion to watch for their events
    // possibly starting a new minion if necessary.
    // FIXME TODO

    // Reply to javascript with the person which will be set in the store
    Ok(person)
}
