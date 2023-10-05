use crate::comms::{ToMinionMessage, ToMinionPayload, ToMinionPayloadDetail, ToOverlordMessage};
use crate::dm_channel::DmChannel;
use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{EventDelegation, EventKind, Id, PublicKey, PublicKeyHex, RelayUrl, Unixtime};
use parking_lot::RwLock;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::task;

/// Kinds of feeds, with configuration parameteers
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
    DmChat(DmChannel),
}

impl std::fmt::Display for FeedKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeedKind::DmChat(channel) => write!(f, "{}", channel.name()),
            FeedKind::Followed(_) => write!(f, "Following"),
            FeedKind::Inbox(_) => write!(f, "Inbox"),
            FeedKind::Thread {
                id: _,
                referenced_by: _,
                author: _,
            } => write!(f, "Thread"),
            FeedKind::Person(_) => write!(f, "Person"),
        }
    }
}

/// The system that computes feeds as an ordered list of event Ids.
pub struct Feed {
    /// Consumers of gossip-lib should only read this, not write to it.
    /// It will be true if the feed is being recomputed.
    pub recompute_lock: AtomicBool,

    current_feed_kind: RwLock<FeedKind>,

    followed_feed: RwLock<Vec<Id>>,
    inbox_feed: RwLock<Vec<Id>>,
    person_feed: RwLock<Vec<Id>>,
    dm_chat_feed: RwLock<Vec<Id>>,

    // We only recompute the feed at specified intervals (or when they switch)
    interval_ms: RwLock<u32>,
    last_computed: RwLock<Option<Instant>>,

    thread_parent: RwLock<Option<Id>>,
}

impl Default for Feed {
    fn default() -> Self {
        Self::new()
    }
}

impl Feed {
    pub(crate) fn new() -> Feed {
        Feed {
            recompute_lock: AtomicBool::new(false),
            current_feed_kind: RwLock::new(FeedKind::Followed(false)),
            followed_feed: RwLock::new(Vec::new()),
            inbox_feed: RwLock::new(Vec::new()),
            person_feed: RwLock::new(Vec::new()),
            dm_chat_feed: RwLock::new(Vec::new()),
            interval_ms: RwLock::new(10000), // Every 10 seconds, until we load from settings
            last_computed: RwLock::new(None),
            thread_parent: RwLock::new(None),
        }
    }

    fn unlisten(&self) {
        let feed_kind = self.current_feed_kind.read().to_owned();

        // If not in the Thread feed
        if !matches!(feed_kind, FeedKind::Thread { .. }) {
            // Stop listening to Thread events
            let _ = GLOBALS.to_minions.send(ToMinionMessage {
                target: "all".to_string(),
                payload: ToMinionPayload {
                    job_id: 0,
                    detail: ToMinionPayloadDetail::UnsubscribeThreadFeed,
                },
            });
        }

        // If not in the Person feed
        if !matches!(feed_kind, FeedKind::Person(_)) {
            // Stop listening to Person events
            let _ = GLOBALS.to_minions.send(ToMinionMessage {
                target: "all".to_string(),
                payload: ToMinionPayload {
                    job_id: 0,
                    detail: ToMinionPayloadDetail::UnsubscribePersonFeed,
                },
            });
        }
    }

    /// Change the feed to the main `followed` feed
    pub fn set_feed_to_followed(&self, with_replies: bool) {
        // We are always subscribed to the general feed. Don't resubscribe here
        // because it won't have changed, but the relays will shower you with
        // all those events again.
        *self.current_feed_kind.write() = FeedKind::Followed(with_replies);
        *self.thread_parent.write() = None;

        // Recompute as they switch
        self.sync_recompute();

        self.unlisten();
    }

    /// Change the feed to the user's `inbox`
    pub fn set_feed_to_inbox(&self, indirect: bool) {
        *self.current_feed_kind.write() = FeedKind::Inbox(indirect);
        *self.thread_parent.write() = None;

        // Recompute as they switch
        self.sync_recompute();

        self.unlisten();
    }

    /// Change the feed to a thread
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

        self.unlisten();

        // Listen for Thread events
        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::SetThreadFeed {
            id,
            referenced_by,
            relays,
            author,
        });
    }

    /// Change the feed to a particular person's notes
    pub fn set_feed_to_person(&self, pubkey: PublicKey) {
        *self.current_feed_kind.write() = FeedKind::Person(pubkey);
        *self.thread_parent.write() = None;

        // Recompute as they switch
        self.sync_recompute();

        self.unlisten();

        // Listen for Person events
        let _ = GLOBALS.to_minions.send(ToMinionMessage {
            target: "all".to_string(),
            payload: ToMinionPayload {
                job_id: 0,
                detail: ToMinionPayloadDetail::SubscribePersonFeed(pubkey),
            },
        });
    }

    /// Change the feed to a DmChat channel
    pub fn set_feed_to_dmchat(&self, channel: DmChannel) {
        *self.current_feed_kind.write() = FeedKind::DmChat(channel.clone());
        *self.thread_parent.write() = None;

        // Recompute as they switch
        self.sync_recompute();

        self.unlisten();

        // Listen for DmChat channel events
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::SetDmChannel(channel));
    }

    /// Get the kind of the current feed
    pub fn get_feed_kind(&self) -> FeedKind {
        self.current_feed_kind.read().to_owned()
    }

    /// Read the followed feed
    pub fn get_followed(&self) -> Vec<Id> {
        self.sync_maybe_periodic_recompute();
        self.followed_feed.read().clone()
    }

    /// Read the inbox
    pub fn get_inbox(&self) -> Vec<Id> {
        self.sync_maybe_periodic_recompute();
        self.inbox_feed.read().clone()
    }

    /// Read the person feed
    pub fn get_person_feed(&self) -> Vec<Id> {
        self.sync_maybe_periodic_recompute();
        self.person_feed.read().clone()
    }

    /// Read the DmChat feed
    pub fn get_dm_chat_feed(&self) -> Vec<Id> {
        self.sync_maybe_periodic_recompute();
        self.dm_chat_feed.read().clone()
    }

    /// Get the parent of the current thread feed.
    /// The children should be recursively found via `GLOBALS.storage.get_replies(id)`
    pub fn get_thread_parent(&self) -> Option<Id> {
        self.sync_maybe_periodic_recompute();
        *self.thread_parent.read()
    }

    /// Overlord climbs and sets this
    pub(crate) fn set_thread_parent(&self, id: Id) {
        *self.thread_parent.write() = Some(id);
    }

    /// This recomputes only if periodic recomputation is enabled, and it has been
    /// at least one period since the last (for any reason) recomputation.
    pub(crate) fn sync_maybe_periodic_recompute(&self) {
        // Only if we recompute periodically
        if !GLOBALS.storage.read_setting_recompute_feed_periodically() {
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

    /// Recompute the feed
    ///
    /// This may happen periodically based on settings. But when a user changes feed, it
    /// is useful to recompute it right away.
    pub fn sync_recompute(&self) {
        task::spawn(async move {
            if let Err(e) = GLOBALS.feed.recompute().await {
                tracing::error!("{}", e);
            }
        });
    }

    pub(crate) async fn recompute(&self) -> Result<(), Error> {
        // If some other process is already recomputing, just return as if
        // the recompute was successful.  Otherwise set to true.
        if self.recompute_lock.fetch_or(true, Ordering::Relaxed) {
            return Ok(());
        }

        *self.last_computed.write() = Some(Instant::now());

        // Copy some values from settings
        let feed_recompute_interval_ms = GLOBALS.storage.read_setting_feed_recompute_interval_ms();

        let kinds_with_dms = feed_displayable_event_kinds(true);
        let kinds_without_dms = feed_displayable_event_kinds(false);

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
                let mut followed_pubkeys: Vec<PublicKey> = GLOBALS.people.get_followed_pubkeys();

                if let Some(pubkey) = GLOBALS.signer.public_key() {
                    followed_pubkeys.push(pubkey); // add the user
                }

                let since = now - Duration::from_secs(GLOBALS.storage.read_setting_feed_chunk());

                // FIXME we don't include delegated events. We should look for all events
                // delegated to people we follow and include those in the feed too.

                let followed_events: Vec<Id> = GLOBALS
                    .storage
                    .find_events(
                        &kinds_without_dms,
                        &followed_pubkeys, // pubkeys
                        Some(since),
                        |e| {
                            e.created_at <= now // no future events
                                && e.kind != EventKind::EncryptedDirectMessage // no DMs
                                && e.kind != EventKind::DmChat // no DMs
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
                    // Unfortunately it is expensive to find all events referencing
                    // any of my events, and we don't have such an index.
                    //
                    // so for now we rely on the fact that replies are supposed to
                    // 'p' tag the authors of people up the chain (see last paragraph
                    // of NIP-10)

                    let my_event_ids: HashSet<Id> = GLOBALS.storage.find_event_ids(
                        &kinds_with_dms,
                        &[my_pubkey], // pubkeys
                        None,         // since
                    )?;

                    let since =
                        now - Duration::from_secs(GLOBALS.storage.read_setting_replies_chunk());

                    let my_pubkeyhex: PublicKeyHex = my_pubkey.into();

                    let inbox_events: Vec<Id> = GLOBALS
                        .storage
                        .find_tagged_events(
                            "p",
                            Some(my_pubkeyhex.as_str()),
                            |e| {
                                if e.created_at < since || e.created_at > now {
                                    return false;
                                }
                                if ! kinds_with_dms.contains(&e.kind) {
                                    return false;
                                }
                                if dismissed.contains(&e.id) {
                                    return false;
                                }
                                if e.kind == EventKind::GiftWrap
                                    || e.kind == EventKind::EncryptedDirectMessage
                                {
                                    return true;
                                }

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
                                    // Include if it directly references me in the content
                                    e.people_referenced_in_content()
                                        .iter()
                                        .any(|p| *p == my_pubkey)
                                }
                            },
                            true,
                        )?
                        .iter()
                        .map(|e| e.id)
                        .collect();

                    *self.inbox_feed.write() = inbox_events;
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
                let since =
                    now - Duration::from_secs(GLOBALS.storage.read_setting_person_feed_chunk());

                let pphex: PublicKeyHex = person_pubkey.into();

                let filter = |e: &Event| {
                    if dismissed.contains(&e.id) {
                        return false;
                    }
                    if !kinds_without_dms.contains(&e.kind) {
                        return false;
                    }
                    true
                };

                let mut events: Vec<Event> = GLOBALS
                    .storage
                    .find_events(
                        &kinds_without_dms,
                        &[person_pubkey],
                        Some(since),
                        filter,
                        false,
                    )?
                    .iter()
                    .chain(
                        GLOBALS
                            .storage
                            .find_tagged_events("delegation", Some(pphex.as_str()), filter, false)?
                            .iter(),
                    )
                    .map(|e| e.to_owned())
                    .collect();

                events.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));

                let events: Vec<Id> = events.iter().map(|e| e.id).collect();

                *self.person_feed.write() = events;
            }
            FeedKind::DmChat(channel) => {
                let ids = GLOBALS.storage.dm_events(&channel)?;
                *self.dm_chat_feed.write() = ids;
            }
        }

        self.recompute_lock.store(false, Ordering::Relaxed);

        Ok(())
    }
}

pub fn enabled_event_kinds() -> Vec<EventKind> {
    let reactions = GLOBALS.storage.read_setting_reactions();
    let reposts = GLOBALS.storage.read_setting_reposts();
    let show_long_form = GLOBALS.storage.read_setting_show_long_form();
    let direct_messages = GLOBALS.storage.read_setting_direct_messages();
    let enable_zap_receipts = GLOBALS.storage.read_setting_enable_zap_receipts();

    EventKind::iter()
        .filter(|k| {
            ((*k != EventKind::Reaction) || reactions)
                && ((*k != EventKind::Repost) || reposts)
                && ((*k != EventKind::LongFormContent) || show_long_form)
                && ((*k != EventKind::EncryptedDirectMessage) || direct_messages)
                && ((*k != EventKind::DmChat) || direct_messages)
                && ((*k != EventKind::GiftWrap) || direct_messages)
                && ((*k != EventKind::Zap) || enable_zap_receipts)
        })
        .collect()
}

pub fn feed_related_event_kinds(dms: bool) -> Vec<EventKind> {
    enabled_event_kinds()
        .drain(..)
        .filter(|k| {
            (k.is_feed_related() || *k == EventKind::GiftWrap)
                && (dms || (*k != EventKind::EncryptedDirectMessage && *k != EventKind::DmChat))
        })
        .collect()
}

pub fn feed_displayable_event_kinds(dms: bool) -> Vec<EventKind> {
    enabled_event_kinds()
        .drain(..)
        .filter(|k| {
            (k.is_feed_displayable() || *k == EventKind::GiftWrap)
                && (dms || (*k != EventKind::EncryptedDirectMessage && *k != EventKind::DmChat))
        })
        .collect()
}

pub fn feed_augment_event_kinds() -> Vec<EventKind> {
    enabled_event_kinds()
        .drain(..)
        .filter(|k| k.augments_feed_related())
        .collect()
}
