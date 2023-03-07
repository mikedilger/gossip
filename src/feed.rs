use crate::comms::{ToMinionMessage, ToMinionPayload, ToOverlordMessage};
use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Event, EventKind, Id, PublicKeyHex, Unixtime};
use parking_lot::RwLock;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::task;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeedKind {
    Followed(bool), // with replies
    Inbox(bool),    // indirect
    Thread { id: Id, referenced_by: Id },
    Person(PublicKeyHex),
}

pub struct Feed {
    pub switched_and_recomputing: AtomicBool,

    current_feed_kind: RwLock<FeedKind>,

    followed_feed: RwLock<Vec<Id>>,
    inbox_feed: RwLock<Vec<Id>>,

    // We only recompute the feed at specified intervals (or when they switch)
    interval_ms: RwLock<u32>,
    last_computed: RwLock<Instant>,

    thread_parent: RwLock<Option<Id>>,
}

impl Feed {
    pub fn new() -> Feed {
        Feed {
            switched_and_recomputing: AtomicBool::new(false),
            current_feed_kind: RwLock::new(FeedKind::Followed(false)),
            followed_feed: RwLock::new(Vec::new()),
            inbox_feed: RwLock::new(Vec::new()),
            interval_ms: RwLock::new(1000), // Every second, until we load from settings
            last_computed: RwLock::new(Instant::now()),
            thread_parent: RwLock::new(None),
        }
    }

    pub fn set_feed_to_followed(&self, with_replies: bool) {
        // We are always subscribed to the general feed. Don't resubscribe here
        // because it won't have changed, but the relays will shower you with
        // all those events again.
        *self.current_feed_kind.write() = FeedKind::Followed(with_replies);
        *self.thread_parent.write() = None;

        // Recompute as they switch
        self.switched_and_recomputing.store(true, Ordering::Relaxed);
        task::spawn(async move {
            let now = Instant::now();
            *GLOBALS.feed.last_computed.write() = now;
            if let Err(e) = GLOBALS.feed.recompute().await {
                tracing::error!("{}", e);
            }
        });

        // When going to Followed or Inbox, we stop listening for Thread/Person events
        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload::UnsubscribeThreadFeed,
        });
        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload::UnsubscribePersonFeed,
        });
    }

    pub fn set_feed_to_inbox(&self, indirect: bool) {
        *self.current_feed_kind.write() = FeedKind::Inbox(indirect);
        *self.thread_parent.write() = None;

        // Recompute as they switch
        self.switched_and_recomputing.store(true, Ordering::Relaxed);
        task::spawn(async move {
            let now = Instant::now();
            *GLOBALS.feed.last_computed.write() = now;
            if let Err(e) = GLOBALS.feed.recompute().await {
                tracing::error!("{}", e);
            }
        });

        // When going to Followed or Inbox, we stop listening for Thread/Person events
        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload::UnsubscribeThreadFeed,
        });
        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload::UnsubscribePersonFeed,
        });
    }

    pub fn set_feed_to_thread(&self, id: Id, referenced_by: Id) {
        *self.current_feed_kind.write() = FeedKind::Thread { id, referenced_by };

        // Parent starts with the post itself
        // Overlord will climb it, and recompute will climb it
        *self.thread_parent.write() = Some(id);

        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload::UnsubscribePersonFeed,
        });
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

    pub fn get_followed(&self) -> Vec<Id> {
        self.maybe_recompute(); // FIXME only do so on a button press.
        self.followed_feed.read().clone()
    }

    pub fn get_inbox(&self) -> Vec<Id> {
        self.maybe_recompute(); // FIXME only do so on a button press.
        self.inbox_feed.read().clone()
    }

    pub fn get_person_feed(&self, person: PublicKeyHex) -> Vec<Id> {
        let enable_reposts = GLOBALS.settings.read().reposts;

        self.maybe_recompute();
        let mut events: Vec<Event> = GLOBALS
            .events
            .iter()
            .map(|r| r.value().to_owned())
            .filter(|e| {
                e.kind == EventKind::TextNote
                    || e.kind == EventKind::EncryptedDirectMessage
                    || (enable_reposts && (e.kind == EventKind::Repost))
            })
            .filter(|e| e.pubkey.as_hex_string() == person.as_str())
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
                *GLOBALS.feed.last_computed.write() = now;
                if let Err(e) = GLOBALS.feed.recompute().await {
                    tracing::error!("{}", e);
                }
            });
        }
    }

    pub async fn recompute(&self) -> Result<(), Error> {
        let settings = GLOBALS.settings.read().clone();
        *self.interval_ms.write() = settings.feed_recompute_interval_ms;

        // Filter further for the general feed
        let dismissed = GLOBALS.dismissed.read().await.clone();
        let now = Unixtime::now().unwrap();

        let current_feed_kind = self.current_feed_kind.read().to_owned();
        match current_feed_kind {
            FeedKind::Followed(with_replies) => {
                let mut followed_pubkeys = GLOBALS.people.get_followed_pubkeys();
                if let Some(pubkey) = GLOBALS.signer.public_key() {
                    followed_pubkeys.push(pubkey.into()); // add the user
                }

                let mut followed_events: Vec<(Unixtime, Id)> = GLOBALS
                    .events
                    .iter()
                    .map(|r| r.value().to_owned())
                    .filter(|e| e.created_at <= now) // no future events
                    .filter(|e| {
                        // feed related
                        e.kind == EventKind::TextNote
                            || (settings.reposts && (e.kind == EventKind::Repost))
                    })
                    .filter(|e| !dismissed.contains(&e.id)) // not dismissed
                    .filter(|e| {
                        if !with_replies {
                            !matches!(e.replies_to(), Some((_id, _))) // is not a reply
                        } else {
                            true
                        }
                    })
                    .filter(|e| followed_pubkeys.contains(&e.pubkey.into())) // someone we follow
                    .map(|e| (e.created_at, e.id))
                    .collect();
                followed_events.sort_by(|a, b| b.0.cmp(&a.0));
                *self.followed_feed.write() = followed_events.iter().map(|e| e.1).collect();
            }
            FeedKind::Inbox(with_replies) => {
                if let Some(my_pubkey) = GLOBALS.signer.public_key() {
                    let events: Vec<Event> = GLOBALS
                        .events
                        .iter()
                        .map(|r| r.value().to_owned())
                        .filter(|e| e.created_at <= now) // no future events
                        .filter(|e| {
                            // feed related
                            e.kind == EventKind::TextNote
                                || e.kind == EventKind::EncryptedDirectMessage
                                || (settings.reposts && (e.kind == EventKind::Repost))
                        })
                        .filter(|e| !dismissed.contains(&e.id)) // not dismissed
                        .collect();

                    let my_event_ids: HashSet<Id> = events
                        .iter()
                        .filter_map(|e| {
                            if e.pubkey == my_pubkey {
                                Some(e.id)
                            } else {
                                None
                            }
                        })
                        .collect();

                    let mut inbox_events: Vec<(Unixtime, Id)> = events
                        .iter()
                        .filter(|e| {
                            // Don't include my own posts
                            if e.pubkey == my_pubkey {
                                return false;
                            }

                            // Include if it directly replies to one of my events
                            // FIXME: maybe try replies_to_ancestors to go deeper
                            if let Some((id, _)) = e.replies_to() {
                                if my_event_ids.contains(&id) {
                                    return true;
                                }
                            }

                            if with_replies {
                                // Include if it tags me
                                e.people().iter().any(|(p, _, _)| *p == my_pubkey.into())
                            } else {
                                if e.kind != EventKind::EncryptedDirectMessage {
                                    // Include if it directly references me in the content
                                    e.referenced_people()
                                        .iter()
                                        .any(|(p, _, _)| *p == my_pubkey.into())
                                } else {
                                    false
                                }
                            }
                        })
                        .map(|e| (e.created_at, e.id))
                        .collect();
                    inbox_events.sort_by(|a, b| b.0.cmp(&a.0));
                    *self.inbox_feed.write() = inbox_events.iter().map(|e| e.1).collect();
                }
            }
            FeedKind::Thread { .. } => {
                // Potentially update thread parent to a higher parent
                let maybe_tp = *self.thread_parent.read();
                if let Some(tp) = maybe_tp {
                    if let Some(new_tp) = GLOBALS.events.get_highest_local_parent(&tp).await? {
                        if new_tp != tp {
                            *self.thread_parent.write() = Some(new_tp);
                        }
                    }
                }
            }
            _ => {}
        }

        self.switched_and_recomputing
            .store(false, Ordering::Relaxed);

        Ok(())
    }
}
