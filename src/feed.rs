use crate::comms::{ToMinionMessage, ToMinionPayload, ToMinionPayloadDetail, ToOverlordMessage};
use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{EventDelegation, EventKind, Id, PublicKey, RelayUrl, Unixtime};
use parking_lot::RwLock;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::task;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeedKind {
    Followed(bool), // with replies
    Inbox(bool),    // indirect
    Thread {
        id: Id,
        referenced_by: Id,
        author: Option<PublicKey>,
    },
    Person(PublicKey),
}

pub struct Feed {
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
            payload: ToMinionPayload {
                job_id: 0,
                detail: ToMinionPayloadDetail::UnsubscribeThreadFeed,
            },
        });
        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload {
                job_id: 0,
                detail: ToMinionPayloadDetail::UnsubscribePersonFeed,
            },
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
            payload: ToMinionPayload {
                job_id: 0,
                detail: ToMinionPayloadDetail::UnsubscribeThreadFeed,
            },
        });
        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload {
                job_id: 0,
                detail: ToMinionPayloadDetail::UnsubscribePersonFeed,
            },
        });
    }

    pub fn set_feed_to_thread(
        &self,
        id: Id,
        referenced_by: Id,
        relays: Vec<RelayUrl>,
        author: Option<PublicKey>,
    ) {
        *self.current_feed_kind.write() = FeedKind::Thread {
            id,
            referenced_by,
            author,
        };

        // Parent starts with the post itself
        // Overlord will climb it, and recompute will climb it
        *self.thread_parent.write() = Some(id);

        // Recompute as they switch
        self.sync_recompute();

        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload {
                job_id: 0,
                detail: ToMinionPayloadDetail::UnsubscribePersonFeed,
            },
        });
        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::SetThreadFeed(
            id,
            referenced_by,
            relays,
            author,
        ));
    }

    pub fn set_feed_to_person(&self, pubkey: PublicKey) {
        *self.current_feed_kind.write() = FeedKind::Person(pubkey);
        *self.thread_parent.write() = None;

        // Recompute as they switch
        self.sync_recompute();

        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload {
                job_id: 0,
                detail: ToMinionPayloadDetail::UnsubscribeThreadFeed,
            },
        });
        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload {
                job_id: 0,
                detail: ToMinionPayloadDetail::SubscribePersonFeed(pubkey),
            },
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
        // If some other process is already recomputing, just return as if
        // the recompute was successful.  Otherwise set to true.
        if self.recompute_lock.fetch_or(true, Ordering::Relaxed) {
            return Ok(());
        }

        *self.last_computed.write() = Some(Instant::now());

        // Copy some values from settings
        let feed_recompute_interval_ms = GLOBALS.settings.read().feed_recompute_interval_ms;

        let kinds = GLOBALS.settings.read().feed_related_event_kinds();

        // We only need to set this the first time, but has to be after
        // settings is loaded (can't be in new()).  Doing it every time is
        // ok because it is more reactive to changes to the setting.
        *self.interval_ms.write() = feed_recompute_interval_ms;

        // Filter further for the general feed
        let dismissed = GLOBALS.dismissed.read().await.clone();
        let now = Unixtime::now().unwrap();
        let one_year_ago = now - Duration::new(60 * 60 * 24 * 365, 0);

        let current_feed_kind = self.current_feed_kind.read().to_owned();
        match current_feed_kind {
            FeedKind::Followed(with_replies) => {
                let mut followed_pubkeys: Vec<PublicKey> = GLOBALS.people.get_followed_pubkeys();

                if let Some(pubkey) = GLOBALS.signer.public_key() {
                    followed_pubkeys.push(pubkey); // add the user
                }

                let since = now - Duration::from_secs(GLOBALS.settings.read().feed_chunk);

                let followed_events: Vec<Id> = GLOBALS
                    .storage
                    .find_events(
                        &kinds,            // kinds
                        &followed_pubkeys, // pubkeys
                        Some(since),
                        |e| {
                            e.created_at <= now // no future events
                                && e.kind != EventKind::EncryptedDirectMessage // no DMs
                                && !e.kind.augments_feed_related() // no augments
                                && !dismissed.contains(&e.id) // not dismissed
                                && if !with_replies {
                                    !matches!(e.replies_to(), Some((_id, _))) // is not a reply
                                } else {
                                    true
                                }
                        },
                        true,
                    )?
                    .iter()
                    .map(|e| e.id)
                    .collect();

                *self.followed_feed.write() = followed_events;
            }
            FeedKind::Inbox(indirect) => {
                if let Some(my_pubkey) = GLOBALS.signer.public_key() {
                    let my_event_ids: HashSet<Id> = GLOBALS
                        .storage
                        .find_events(
                            &kinds,       // kinds
                            &[my_pubkey], // pubkeys
                            None,         // since
                            |_| true,
                            false,
                        )?
                        .iter()
                        .map(|e| e.id)
                        .collect();

                    let inbox_events: Vec<(Unixtime, Id)> = GLOBALS
                        .storage
                        .find_events(
                            &kinds,             // kinds
                            &[],                // pubkeys
                            Some(one_year_ago), // since
                            |e| {
                                if e.created_at > now {
                                    return false;
                                } // no future events
                                if e.kind.augments_feed_related() {
                                    return false;
                                } // no augments
                                if dismissed.contains(&e.id) {
                                    return false;
                                } // not dismissed
                                if e.pubkey == my_pubkey {
                                    return false;
                                } // not self-authored

                                // Include if it directly replies to one of my events
                                if let Some((id, _)) = e.replies_to() {
                                    if my_event_ids.contains(&id) {
                                        return true;
                                    }
                                }

                                if indirect {
                                    // Include if it tags me
                                    e.people().iter().any(|(p, _, _)| *p == my_pubkey.into())
                                } else {
                                    if e.kind == EventKind::EncryptedDirectMessage {
                                        true
                                    } else {
                                        // Include if it directly references me in the content
                                        e.people_referenced_in_content()
                                            .iter()
                                            .any(|p| *p == my_pubkey)
                                    }
                                }
                            },
                            true,
                        )?
                        .iter()
                        .map(|e| (e.created_at, e.id))
                        .collect();

                    *self.inbox_feed.write() = inbox_events.iter().map(|e| e.1).collect();
                }
            }
            FeedKind::Thread { .. } => {
                // Potentially update thread parent to a higher parent
                let maybe_tp = *self.thread_parent.read();
                if let Some(tp) = maybe_tp {
                    if let Some(new_tp) = GLOBALS.storage.get_highest_local_parent_event_id(tp)? {
                        if new_tp != tp {
                            *self.thread_parent.write() = Some(new_tp);
                        }
                    }
                }
            }
            FeedKind::Person(person_pubkey) => {
                let events: Vec<(Unixtime, Id)> = GLOBALS
                    .storage
                    .find_events(
                        &kinds,             // feed kinds
                        &[], // any person (due to delegation condition) // FIXME GINA
                        Some(one_year_ago), // one year ago
                        |e| {
                            if e.kind.augments_feed_related() {
                                return false;
                            } // not augments
                            if dismissed.contains(&e.id) {
                                return false;
                            } // not dismissed
                            if e.pubkey == person_pubkey {
                                true
                            } else {
                                if let EventDelegation::DelegatedBy(pk) = e.delegation() {
                                    pk == person_pubkey
                                } else {
                                    false
                                }
                            }
                        },
                        true,
                    )?
                    .iter()
                    .map(|e| (e.created_at, e.id))
                    .collect();

                *self.person_feed.write() = events.iter().map(|e| e.1).collect();
            }
        }

        self.recompute_lock.store(false, Ordering::Relaxed);

        Ok(())
    }
}
