use crate::comms::{ToMinionMessage, ToMinionPayload, ToOverlordMessage};
use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{EventKind, Id, PublicKeyHex, RelayUrl, Unixtime};
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
    /// Indicates that feed events have been loaded from the DB into [GLOBALS.events], and therefore
    /// [Feed] rendering methods have data on which they can work.
    ///
    /// Calling [Feed::sync_maybe_periodic_recompute] when this is false will simply return.
    ///
    /// Calling [Feed::recompute] when this is false with throw an error.
    pub ready: AtomicBool,

    pub recompute_lock: AtomicBool,

    current_feed_kind: RwLock<FeedKind>,

    followed_feed: RwLock<Vec<Id>>,
    inbox_feed: RwLock<Vec<Id>>,
    person_feed: RwLock<Vec<Id>>,

    // We only recompute the feed at specified intervals (or when they switch)
    interval_ms: RwLock<u32>,
    last_computed: RwLock<Option<Instant>>,

    thread_parent: RwLock<Option<Id>>,
}

impl Feed {
    pub fn new() -> Feed {
        Feed {
            ready: AtomicBool::new(false),
            recompute_lock: AtomicBool::new(false),
            current_feed_kind: RwLock::new(FeedKind::Followed(false)),
            followed_feed: RwLock::new(Vec::new()),
            inbox_feed: RwLock::new(Vec::new()),
            person_feed: RwLock::new(Vec::new()),
            interval_ms: RwLock::new(1000), // Every second, until we load from settings
            last_computed: RwLock::new(None),
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
        self.sync_recompute();

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
        self.sync_recompute();

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

    pub fn set_feed_to_thread(&self, id: Id, referenced_by: Id, relays: Vec<RelayUrl>) {
        *self.current_feed_kind.write() = FeedKind::Thread { id, referenced_by };

        // Parent starts with the post itself
        // Overlord will climb it, and recompute will climb it
        *self.thread_parent.write() = Some(id);

        // Recompute as they switch
        self.sync_recompute();

        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload::UnsubscribePersonFeed,
        });
        let _ =
            GLOBALS
                .to_overlord
                .send(ToOverlordMessage::SetThreadFeed(id, referenced_by, relays));
    }

    pub fn set_feed_to_person(&self, pubkey: PublicKeyHex) {
        *self.current_feed_kind.write() = FeedKind::Person(pubkey.clone());
        *self.thread_parent.write() = None;

        // Recompute as they switch
        self.sync_recompute();

        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload::UnsubscribeThreadFeed,
        });
        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload::SubscribePersonFeed(pubkey),
        });
    }

    pub fn get_feed_kind(&self) -> FeedKind {
        self.current_feed_kind.read().to_owned()
    }

    pub fn get_followed(&self) -> Vec<Id> {
        self.sync_maybe_periodic_recompute();
        self.followed_feed.read().clone()
    }

    pub fn get_inbox(&self) -> Vec<Id> {
        self.sync_maybe_periodic_recompute();
        self.inbox_feed.read().clone()
    }

    pub fn get_person_feed(&self) -> Vec<Id> {
        self.sync_maybe_periodic_recompute();
        self.person_feed.read().clone()
    }

    pub fn get_thread_parent(&self) -> Option<Id> {
        self.sync_maybe_periodic_recompute();
        *self.thread_parent.read()
    }

    // Overlord climbs and sets this
    pub fn set_thread_parent(&self, id: Id) {
        *self.thread_parent.write() = Some(id);
    }

    // This recomputes only if periodic recomputation is enabled, and it has been
    // at least one period since the last (for any reason) recomputation.
    pub fn sync_maybe_periodic_recompute(&self) {
        // Only if we recompute periodically
        if !GLOBALS.settings.read().recompute_feed_periodically {
            return;
        }

        if !self.ready.load(Ordering::Relaxed) {
            return;
        }

        let now = Instant::now();
        let recompute = self
            .last_computed
            .read()
            .map(|last_computed| {
                last_computed + Duration::from_millis(*self.interval_ms.read() as u64) < now
            })
            .unwrap_or(true);
        if recompute {
            self.sync_recompute();
        }
    }

    pub fn sync_recompute(&self) {
        task::spawn(async move {
            if let Err(e) = GLOBALS.feed.recompute().await {
                tracing::error!("{}", e);
            }
        });
    }

    pub async fn recompute(&self) -> Result<(), Error> {
        if !self.ready.load(Ordering::Relaxed) {
            return Err("Feed is not yet ready")?;
        }

        // If some other process is already recomputing, just return as if
        // the recompute was successful.  Otherwise set to true.
        if self.recompute_lock.fetch_or(true, Ordering::Relaxed) {
            return Ok(());
        }

        *self.last_computed.write() = Some(Instant::now());

        // Copy some values from settings
        let (feed_recompute_interval_ms, reposts) = {
            let settings = GLOBALS.settings.read();
            (settings.feed_recompute_interval_ms, settings.reposts)
        };

        // We only need to set this the first time, but has to be after
        // settings is loaded (can't be in new()).  Doing it every time is
        // ok because it is more reactive to changes to the setting.
        *self.interval_ms.write() = feed_recompute_interval_ms;

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
                        e.kind == EventKind::TextNote || (reposts && (e.kind == EventKind::Repost))
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
            FeedKind::Inbox(indirect) => {
                if let Some(my_pubkey) = GLOBALS.signer.public_key() {
                    let my_event_ids: HashSet<Id> = GLOBALS
                        .events
                        .iter()
                        .filter_map(|e| {
                            if e.value().pubkey == my_pubkey {
                                Some(e.value().id)
                            } else {
                                None
                            }
                        })
                        .collect();

                    let mut inbox_events: Vec<(Unixtime, Id)> = GLOBALS
                        .events
                        .iter()
                        .filter(|e| e.value().created_at <= now) // no future events
                        .filter(|e| {
                            // feed related
                            e.value().kind == EventKind::TextNote
                                || e.value().kind == EventKind::EncryptedDirectMessage
                                || (reposts && (e.value().kind == EventKind::Repost))
                        })
                        .filter(|e| !dismissed.contains(&e.value().id)) // not dismissed
                        .filter(|e| e.value().pubkey != my_pubkey) // not self-authored
                        .filter(|e| {
                            // Include if it directly replies to one of my events
                            if let Some((id, _)) = e.value().replies_to() {
                                if my_event_ids.contains(&id) {
                                    return true;
                                }
                            }

                            if indirect {
                                // Include if it tags me
                                e.value()
                                    .people()
                                    .iter()
                                    .any(|(p, _, _)| *p == my_pubkey.into())
                            } else {
                                if e.value().kind == EventKind::EncryptedDirectMessage {
                                    true
                                } else {
                                    // Include if it directly references me in the content
                                    e.value()
                                        .referenced_people()
                                        .iter()
                                        .any(|(p, _, _)| *p == my_pubkey.into())
                                }
                            }
                        })
                        .map(|e| (e.value().created_at, e.value().id))
                        .collect();

                    // Sort
                    inbox_events.sort_unstable_by(|a, b| b.0.cmp(&a.0));

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
            FeedKind::Person(person_pubkey) => {
                let mut events: Vec<(Unixtime, Id)> = GLOBALS
                    .events
                    .iter()
                    .filter(|e| {
                        e.value().kind == EventKind::TextNote
                            || e.value().kind == EventKind::EncryptedDirectMessage
                            || (reposts && (e.value().kind == EventKind::Repost))
                    })
                    .filter(|e| e.value().pubkey.as_hex_string() == person_pubkey.as_str())
                    .filter(|e| !dismissed.contains(&e.value().id)) // not dismissed
                    .map(|e| (e.value().created_at, e.value().id))
                    .collect();

                // Sort
                events.sort_unstable_by(|a, b| b.0.cmp(&a.0));

                *self.person_feed.write() = events.iter().map(|e| e.1).collect();
            }
        }

        self.recompute_lock.store(false, Ordering::Relaxed);

        Ok(())
    }
}
