use crate::comms::{ToMinionMessage, ToMinionPayload};
use crate::globals::GLOBALS;
use nostr_types::PublicKeyHex;
use nostr_types::{Event, EventKind, Id};
use parking_lot::RwLock;
use std::collections::HashSet;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub enum FeedKind {
    General,
    Replies,
    Thread(Id),
    Person(PublicKeyHex),
}

pub struct Feed {
    current_feed_kind: RwLock<FeedKind>,

    general_feed: RwLock<Vec<Id>>,
    replies_feed: RwLock<Vec<Id>>,

    // We only recompute the feed at specified intervals
    interval_ms: RwLock<u32>,
    last_computed: RwLock<Instant>,

    // We track these to update subscriptions on them
    my_event_ids: RwLock<Vec<Id>>,
    followed_event_ids: RwLock<Vec<Id>>,
}

impl Feed {
    pub fn new() -> Feed {
        Feed {
            current_feed_kind: RwLock::new(FeedKind::General),
            general_feed: RwLock::new(Vec::new()),
            replies_feed: RwLock::new(Vec::new()),
            interval_ms: RwLock::new(1000), // Every second, until we load from settings
            last_computed: RwLock::new(Instant::now()),
            my_event_ids: RwLock::new(Vec::new()),
            followed_event_ids: RwLock::new(Vec::new()),
        }
    }

    pub fn set_feed_to_general(&self) {
        // We are always subscribed to the general feed. Don't resubscribe here
        // because it won't have changed, but the relays will shower you with
        // all those events again.
        *self.current_feed_kind.write() = FeedKind::General;
    }

    pub fn set_feed_to_replies(&self) {
        *self.current_feed_kind.write() = FeedKind::Replies;
    }

    pub fn set_feed_to_thread(&self, id: Id) {
        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload::SubscribeThreadFeed(id),
        });
        *self.current_feed_kind.write() = FeedKind::Thread(id);
    }

    pub fn set_feed_to_person(&self, pubkey: PublicKeyHex) {
        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload::SubscribePersonFeed(pubkey.clone()),
        });
        *self.current_feed_kind.write() = FeedKind::Person(pubkey);
    }

    pub fn get_feed_kind(&self) -> FeedKind {
        self.current_feed_kind.read().to_owned()
    }

    pub fn get_general(&self) -> Vec<Id> {
        let now = Instant::now();
        if *self.last_computed.read() + Duration::from_millis(*self.interval_ms.read() as u64) < now
        {
            self.recompute();
            *self.last_computed.write() = now;
        }

        self.general_feed.read().clone()
    }

    pub fn get_replies(&self) -> Vec<Id> {
        let now = Instant::now();
        if *self.last_computed.read() + Duration::from_millis(*self.interval_ms.read() as u64) < now
        {
            self.recompute();
            *self.last_computed.write() = now;
        }

        self.replies_feed.read().clone()
    }

    pub fn get_thread_parent(&self, id: Id) -> Id {
        let mut event = match GLOBALS.events.blocking_read().get(&id).cloned() {
            None => return id,
            Some(e) => e,
        };

        // Try for root
        if let Some((root, _)) = event.replies_to_root() {
            if GLOBALS.events.blocking_read().contains_key(&root) {
                return root;
            }
        }

        // Climb parents as high as we can
        while let Some((parent, _)) = event.replies_to() {
            if let Some(e) = GLOBALS.events.blocking_read().get(&parent) {
                event = e.to_owned();
            } else {
                break;
            }
        }

        // The highest event id we have
        event.id
    }

    pub fn get_person_feed(&self, person: PublicKeyHex) -> Vec<Id> {
        let mut events: Vec<Event> = GLOBALS
            .events
            .blocking_read()
            .iter()
            .map(|(_, e)| e)
            .filter(|e| e.kind == EventKind::TextNote)
            .filter(|e| e.pubkey.as_hex_string() == person.0)
            .filter(|e| !GLOBALS.dismissed.blocking_read().contains(&e.id))
            .map(|e| e.to_owned())
            .collect();

        events.sort_unstable_by(|a, b| b.created_at.cmp(&a.created_at));

        events.iter().map(|e| e.id).collect()
    }

    #[allow(dead_code)]
    pub fn get_my_event_ids(&self) -> Vec<Id> {
        // we assume the main get() happens fast enough to recompute for us.
        self.my_event_ids.read().clone()
    }

    #[allow(dead_code)]
    pub fn get_followed_event_ids(&self) -> Vec<Id> {
        // we assume the main get() happens fast enough to recompute for us.
        self.followed_event_ids.read().clone()
    }

    fn recompute(&self) {
        let settings = GLOBALS.settings.blocking_read().clone();
        *self.interval_ms.write() = settings.feed_recompute_interval_ms;

        let events: Vec<Event> = GLOBALS
            .events
            .blocking_read()
            .iter()
            .map(|(_, e)| e)
            .filter(|e| e.kind == EventKind::TextNote)
            .map(|e| e.to_owned())
            .collect();

        let mut pubkeys = GLOBALS.people.blocking_read().get_followed_pubkeys();
        if let Some(pubkey) = GLOBALS.signer.blocking_read().public_key() {
            pubkeys.push(pubkey.into()); // add the user
        }

        // My event ids
        if let Some(pubkey) = GLOBALS.signer.blocking_read().public_key() {
            *self.my_event_ids.write() = events
                .iter()
                .filter_map(|e| if e.pubkey == pubkey { Some(e.id) } else { None })
                .collect();
        } else {
            *self.my_event_ids.write() = vec![];
        }

        // Followed event ids
        *self.followed_event_ids.write() = events
            .iter()
            .filter_map(|e| {
                if pubkeys.contains(&e.pubkey.into()) {
                    Some(e.id)
                } else {
                    None
                }
            })
            .collect();

        // Filter further for the general feed
        let mut fevents: Vec<Event> = events
            .iter()
            .filter(|e| !GLOBALS.dismissed.blocking_read().contains(&e.id))
            .filter(|e| pubkeys.contains(&e.pubkey.into())) // something we follow
            .cloned()
            .collect();
        fevents.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        *self.general_feed.write() = fevents.iter().map(|e| e.id).collect();

        // Filter differently for the replies feed
        let my_events: HashSet<Id> = self.my_event_ids.read().iter().copied().collect();
        let mut revents: Vec<Event> = events
            .iter()
            .filter(|e| !GLOBALS.dismissed.blocking_read().contains(&e.id))
            .filter(|e| {
                // FIXME: maybe try replies_to_ancestors to go deeper
                if let Some((id, _)) = e.replies_to() {
                    if my_events.contains(&id) {
                        return true;
                    }
                }
                false
            })
            .cloned()
            .collect();
        revents.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        *self.replies_feed.write() = revents.iter().map(|e| e.id).collect();
    }
}
