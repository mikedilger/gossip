use crate::comms::{ToMinionMessage, ToMinionPayload, ToOverlordMessage};
use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Event, EventKind, Id, PublicKeyHex, Unixtime};
use parking_lot::RwLock;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tokio::task;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeedKind {
    General,
    Replies,
    Thread { id: Id, referenced_by: Id },
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

    thread_parent: RwLock<Option<Id>>,
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
            thread_parent: RwLock::new(None),
        }
    }

    pub fn set_feed_to_general(&self) {
        // We are always subscribed to the general feed. Don't resubscribe here
        // because it won't have changed, but the relays will shower you with
        // all those events again.
        *self.current_feed_kind.write() = FeedKind::General;
        *self.thread_parent.write() = None;
        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload::UnsubscribeThreadFeed,
        });
    }

    pub fn set_feed_to_replies(&self) {
        *self.current_feed_kind.write() = FeedKind::Replies;
        *self.thread_parent.write() = None;
        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload::UnsubscribeThreadFeed,
        });
    }

    pub fn set_feed_to_thread(&self, id: Id, referenced_by: Id) {
        *self.current_feed_kind.write() = FeedKind::Thread { id, referenced_by };
        // Parent starts with the post itself
        // Overlord will climb it, and recompute will climb it
        *self.thread_parent.write() = Some(id);
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::SetThreadFeed(id, referenced_by));
    }

    pub fn set_feed_to_person(&self, pubkey: PublicKeyHex) {
        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload::UnsubscribeThreadFeed,
        });
        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload::SubscribePersonFeed(pubkey.clone()),
        });
        *self.current_feed_kind.write() = FeedKind::Person(pubkey);
        *self.thread_parent.write() = None;
    }

    pub fn get_feed_kind(&self) -> FeedKind {
        self.current_feed_kind.read().to_owned()
    }

    pub fn get_general(&self) -> Vec<Id> {
        self.maybe_recompute();
        self.general_feed.read().clone()
    }

    pub fn get_replies(&self) -> Vec<Id> {
        self.maybe_recompute();
        self.replies_feed.read().clone()
    }

    pub fn get_person_feed(&self, person: PublicKeyHex) -> Vec<Id> {
        self.maybe_recompute();
        let mut events: Vec<Event> = GLOBALS
            .events
            .iter()
            .map(|r| r.value().to_owned())
            .filter(|e| e.kind == EventKind::TextNote)
            .filter(|e| e.pubkey.as_hex_string() == person.0)
            .filter(|e| !GLOBALS.dismissed.blocking_read().contains(&e.id))
            .collect();

        events.sort_unstable_by(|a, b| b.created_at.cmp(&a.created_at));

        events.iter().map(|e| e.id).collect()
    }

    pub fn get_thread_parent(&self) -> Option<Id> {
        self.maybe_recompute();
        *self.thread_parent.read()
    }

    // Overlord climbs and sets this
    pub fn set_thread_parent(&self, id: Id) {
        *self.thread_parent.write() = Some(id);
    }

    pub fn maybe_recompute(&self) {
        let now = Instant::now();
        if *self.last_computed.read() + Duration::from_millis(*self.interval_ms.read() as u64) < now
        {
            let now = now;
            task::spawn(async move {
                if let Err(e) = GLOBALS.feed.recompute().await {
                    tracing::error!("{}", e);
                }
                *GLOBALS.feed.last_computed.write() = now;
            });
        }
    }

    pub async fn recompute(&self) -> Result<(), Error> {
        let settings = GLOBALS.settings.read().await.clone();
        *self.interval_ms.write() = settings.feed_recompute_interval_ms;

        let events: Vec<Event> = GLOBALS
            .events
            .iter()
            .map(|r| r.value().to_owned())
            .filter(|e| e.kind == EventKind::TextNote)
            .collect();

        let mut followed_pubkeys = GLOBALS.people.get_followed_pubkeys();
        if let Some(pubkey) = GLOBALS.signer.read().await.public_key() {
            followed_pubkeys.push(pubkey.into()); // add the user
        }

        // My event ids
        if let Some(pubkey) = GLOBALS.signer.read().await.public_key() {
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
                if followed_pubkeys.contains(&e.pubkey.into()) {
                    Some(e.id)
                } else {
                    None
                }
            })
            .collect();

        // Filter further for the general feed
        let now = Unixtime::now().unwrap();
        let dismissed = GLOBALS.dismissed.read().await.clone();

        let mut fevents: Vec<Event> = events
            .iter()
            .filter(|e| !dismissed.contains(&e.id))
            .filter(|e| followed_pubkeys.contains(&e.pubkey.into())) // something we follow
            .filter(|e| e.created_at <= now)
            .cloned()
            .collect();
        fevents.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        *self.general_feed.write() = fevents.iter().map(|e| e.id).collect();

        // Filter differently for the replies feed
        let direct_only = GLOBALS.settings.read().await.direct_replies_only;

        if let Some(my_pubkey) = GLOBALS.signer.read().await.public_key() {
            let my_events: HashSet<Id> = self.my_event_ids.read().iter().copied().collect();
            let mut revents: Vec<Event> = events
                .iter()
                .filter(|e| !dismissed.contains(&e.id))
                .filter(|e| {
                    // Don't include my own posts
                    if e.pubkey == my_pubkey {
                        return false;
                    }

                    // Include if it directly replies to one of my events
                    // FIXME: maybe try replies_to_ancestors to go deeper
                    if let Some((id, _)) = e.replies_to() {
                        if my_events.contains(&id) {
                            return true;
                        }
                    }

                    if direct_only {
                        // Include if it directly references me in the content
                        e.referenced_people()
                            .iter()
                            .any(|(p, _, _)| *p == my_pubkey.into())
                    } else {
                        // Include if it tags me
                        e.people().iter().any(|(p, _, _)| *p == my_pubkey.into())
                    }
                })
                .cloned()
                .collect();
            revents.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            *self.replies_feed.write() = revents.iter().map(|e| e.id).collect();
        }

        // Potentially update thread parent to a higher parent
        let maybe_tp = *self.thread_parent.read();
        if let Some(tp) = maybe_tp {
            if let Some(new_tp) = GLOBALS.events.get_highest_local_parent(&tp).await? {
                if new_tp != tp {
                    *self.thread_parent.write() = Some(new_tp);
                }
            }
        }

        Ok(())
    }
}
