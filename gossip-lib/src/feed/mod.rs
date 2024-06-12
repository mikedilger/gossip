mod feed_kind;
pub use feed_kind::FeedKind;

use crate::comms::{ToMinionMessage, ToMinionPayload, ToMinionPayloadDetail, ToOverlordMessage};
use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::people::PersonList;
use dashmap::DashMap;
use nostr_types::{Event, EventAddr, EventKind, EventReference, Filter, Id, Unixtime};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::task;

/// The system that computes feeds as an ordered list of event Ids.
pub struct Feed {
    /// Consumers of gossip-lib should only read this, not write to it.
    /// It will be true if the feed is being recomputed.
    pub recompute_lock: AtomicBool,

    current_feed_kind: RwLock<FeedKind>,
    current_feed_events: RwLock<Vec<Id>>,
    feed_anchors: DashMap<String, Unixtime>,

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
            current_feed_kind: RwLock::new(FeedKind::List(PersonList::Followed, false)),
            current_feed_events: RwLock::new(Vec::new()),
            feed_anchors: DashMap::new(),
            interval_ms: RwLock::new(10000), // Every 10 seconds, until we load from settings
            last_computed: RwLock::new(None),
            thread_parent: RwLock::new(None),
        }
    }

    /// This changes the window where the feed pulls its events from by backing up the
    /// anchor time to the time of the earliest event currently in the feed.
    //
    /// This doesn't deal with minion subscriptions.
    pub(crate) fn load_more(&self) -> Result<Unixtime, Error> {
        let anchor_key = self.current_feed_kind.read().anchor_key();
        // Load the timestamp of the earliest event in the feed so far
        if let Some(earliest_id) = self.current_feed_events.read().iter().next_back() {
            let earliest_event = GLOBALS.storage.read_event(*earliest_id)?;
            if let Some(event) = earliest_event {
                // Move the anchor back to the earliest event we have so far
                self.feed_anchors.insert(anchor_key, event.created_at);

                // Recompute now to get the storage data
                self.sync_recompute();

                Ok(event.created_at)
            } else {
                Err(ErrorKind::LoadMoreFailed.into())
            }
        } else {
            Err(ErrorKind::LoadMoreFailed.into())
        }
    }

    pub(crate) fn current_anchor(&self) -> Unixtime {
        let anchor_key = self.current_feed_kind.read().anchor_key();
        *self.feed_anchors.get(&anchor_key).unwrap()
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
                    detail: ToMinionPayloadDetail::UnsubscribeReplies,
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

    pub fn switch_feed(&self, feed_kind: FeedKind) {
        // NOTE: do not clear the feed here, or the UI will get an empty feed momentarily
        // and the scroll bar "memory" will be reset to the top.  Let recompute rebuild
        // the feed (called down below)

        let anchor: Unixtime = {
            let anchor_key = feed_kind.anchor_key();
            match self.feed_anchors.get(&anchor_key) {
                Some(refanchor) => *refanchor,
                None => {
                    // Start the feed anchor if it was not yet set
                    let now = Unixtime::now().unwrap();
                    self.feed_anchors.insert(anchor_key, now);
                    now
                }
            }
        };

        // Reset the feed thread
        *self.thread_parent.write() = if let FeedKind::Thread {
            id,
            referenced_by: _,
            author: _,
        } = &feed_kind
        {
            // Parent starts with the post itself
            // Overlord will climb it, and recompute will climb it
            Some(*id)
        } else {
            None
        };

        // Set the feed kind
        *self.current_feed_kind.write() = feed_kind;

        // Recompute as they switch
        self.sync_recompute();

        // Unlisten to the relays
        self.unlisten();

        match &*self.current_feed_kind.read() {
            FeedKind::Thread {
                id,
                referenced_by,
                author,
            } => {
                // Listen for Thread events
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::SetThreadFeed {
                    id: *id,
                    referenced_by: *referenced_by,
                    author: *author,
                });
            }
            FeedKind::Person(pubkey) => {
                // Listen for Person events
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::SetPersonFeed(*pubkey, anchor));
            }
            FeedKind::DmChat(ref dm_channel) => {
                // Listen for DmChat channel events
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::SetDmChannel(dm_channel.clone()));
            }
            _ => (),
        }
    }

    /// Get the kind of the current feed
    pub fn get_feed_kind(&self) -> FeedKind {
        self.current_feed_kind.read().to_owned()
    }

    /// Read the followed feed
    pub fn get_feed_events(&self) -> Vec<Id> {
        self.sync_maybe_periodic_recompute();
        self.current_feed_events.read().clone()
    }

    /// Get the parent of the current thread feed.
    /// The children should be recursively found via `GLOBALS.storage.get_replies(id)`
    pub fn get_thread_parent(&self) -> Option<Id> {
        self.sync_maybe_periodic_recompute();
        *self.thread_parent.read()
    }

    /// When initially changing to the thread feed, the Overlord sets the thread
    /// parent to the highest locally available one (or the event if it is not local)
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
        if self.recompute_lock.fetch_or(true, Ordering::Relaxed) {
            // If other process is already recomputing, just return as if
            // the recompute was successful.
            return Ok(());
        }

        // Copy some values from settings
        let feed_recompute_interval_ms = GLOBALS.storage.read_setting_feed_recompute_interval_ms();

        // We only need to set this the first time, but has to be after
        // settings is loaded (can't be in new()).  Doing it every time is
        // ok because it is more reactive to changes to the setting.
        *self.interval_ms.write() = feed_recompute_interval_ms;

        let anchor: Unixtime = self.current_anchor();

        let current_feed_kind = self.current_feed_kind.read().to_owned();
        match current_feed_kind {
            FeedKind::List(list, with_replies) => {
                let filter = {
                    let mut filter = Filter::new();
                    filter.authors = GLOBALS
                        .storage
                        .get_people_in_list(list)?
                        .drain(..)
                        .map(|(pk, _)| pk.into())
                        .collect();
                    filter.kinds = feed_displayable_event_kinds(false);
                    filter
                };

                let events = if filter.authors.is_empty() {
                    Default::default()
                } else {
                    Self::load_event_range(anchor, filter, with_replies, false, |_| true).await?
                };

                *self.current_feed_events.write() = events;
            }
            FeedKind::Inbox(indirect) => {
                if let Some(my_pubkey) = GLOBALS.identity.public_key() {
                    // Ideally all replies would 'p' tag me (NIP-10)
                    //
                    // But some don't. To see such replies, we would like to look for any
                    // event that replies to any of my events. But it is too expensive to
                    // load all the IDs of all my events and screen for 'e' tags against
                    // them (we used to do that for the most recent of my events, but I
                    // have taken it out).

                    let filter = {
                        let mut filter = Filter::new();
                        filter.kinds = feed_displayable_event_kinds(false);
                        filter.add_tag_value('p', my_pubkey.as_hex_string());
                        filter
                    };

                    // TODO: If event_tag_index had reverse created_at, we could much more
                    //       quickly find inbox messages by using a better prefix scan in
                    //       find_events_by_filter

                    let screen = |e: &Event| {
                        e.pubkey != my_pubkey
                            && (
                                // We can't check against EventReference::Id because we would
                                // have to know all the Ids of my events, which is too much to
                                // check against. BUT we can check for these cheaply:
                                matches!(
                                    e.replies_to(),
                                    Some(EventReference::Addr(EventAddr { author, .. }))
                                        if author == my_pubkey
                                ) || (indirect
                                    || e.people_referenced_in_content()
                                        .iter()
                                        .any(|p| *p == my_pubkey))
                            )
                    };

                    let events =
                        Self::load_event_range(anchor, filter, true, false, screen).await?;
                    *self.current_feed_events.write() = events;
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

                // Thread recompute can be much faster, the above code is pretty cheap
                *self.interval_ms.write() = 500;
            }
            FeedKind::Person(person_pubkey) => {
                let filter = {
                    let mut filter = Filter::new();
                    filter.authors = vec![person_pubkey.into()];
                    filter.kinds = feed_displayable_event_kinds(false);
                    filter
                };

                let events = Self::load_event_range(anchor, filter, true, false, |_| true).await?;

                *self.current_feed_events.write() = events;
            }
            FeedKind::DmChat(channel) => {
                let ids = GLOBALS.storage.dm_events(&channel)?;
                *self.current_feed_events.write() = ids;
            }
        }

        *self.last_computed.write() = Some(Instant::now());
        self.recompute_lock.store(false, Ordering::Relaxed);

        Ok(())
    }

    async fn load_event_range<F>(
        since: Unixtime,
        filter: Filter,
        include_replies: bool,
        include_dms: bool,
        screen: F,
    ) -> Result<Vec<Id>, Error>
    where
        F: Fn(&Event) -> bool,
    {
        let now = Unixtime::now().unwrap();
        let limit = GLOBALS.storage.read_setting_load_more_count() as usize;
        let dismissed = GLOBALS.dismissed.read().await.clone();

        let outer_screen =
            |e: &Event| basic_screen(e, include_replies, include_dms, &dismissed) && screen(e);

        let mut before_filter = filter;
        let mut after_filter = before_filter.clone();

        before_filter.until = Some(since - Duration::from_secs(1));
        before_filter.limit = Some(limit);

        after_filter.since = Some(since);
        after_filter.until = Some(now);

        // FIXME we don't include delegated events.
        /*
        This would screw up the sort:
                    .chain(
                        GLOBALS
                            .storage
                            .find_tagged_events("delegation", Some(pphex.as_str()), screen, false)?
                            .iter(),
                    )
         */

        Ok(GLOBALS
            .storage
            .find_events_by_filter(&after_filter, outer_screen)?
            .iter()
            .map(|e| e.id)
            .chain(
                GLOBALS
                    .storage
                    .find_events_by_filter(&before_filter, outer_screen)?
                    .iter()
                    .map(|e| e.id),
            )
            .collect())
    }
}

#[inline]
fn basic_screen(e: &Event, include_replies: bool, include_dms: bool, dismissed: &[Id]) -> bool {
    let now = Unixtime::now().unwrap();

    e.created_at <= now
        && (include_replies || e.replies_to().is_none())
        && (include_dms
            || (e.kind != EventKind::EncryptedDirectMessage
                && e.kind != EventKind::DmChat
                && e.kind != EventKind::GiftWrap))
        && !dismissed.contains(&e.id)
        && !e.is_annotation()
}

pub fn enabled_event_kinds() -> Vec<EventKind> {
    let reactions = GLOBALS.storage.read_setting_reactions();
    let reposts = GLOBALS.storage.read_setting_reposts();
    let show_long_form = GLOBALS.storage.read_setting_show_long_form();
    let direct_messages = GLOBALS.storage.read_setting_direct_messages();
    let enable_zap_receipts = GLOBALS.storage.read_setting_enable_zap_receipts();

    EventKind::iter()
        .filter(|k| {
            *k == EventKind::Metadata
                || *k == EventKind::TextNote
            //|| *k == EventKind::RecommendRelay
                || *k == EventKind::ContactList
                || ((*k == EventKind::EncryptedDirectMessage) && direct_messages)
                || *k == EventKind::EventDeletion
                || ((*k == EventKind::Repost) && reposts)
                || ((*k == EventKind::Reaction) && reactions)
            //|| *k == EventKind::BadgeAward
            //|| *k == EventKind::Seal // -- never subscribed to
                || ((*k == EventKind::DmChat) && direct_messages)
                || ((*k == EventKind::GenericRepost) && reposts)
            //|| *k == EventKind::ChannelCreation
            //|| *k == EventKind::ChannelMetadata
            //|| *k == EventKind::ChannelMessage
            //|| *k == EventKind::ChannelHideMessage
            //|| *k == EventKind::ChannelMuteUser
            // || *k == EventKind::Timestamp
                || ((*k == EventKind::GiftWrap) && direct_messages)
            // || *k == EventKind::FileMetadata
            // || *k == EventKind::LiveChatMessage
            // || *k == EventKind::Patches
            // || *k == EventKind::GitIssue
            // || *k == EventKind::GitReply
            // || *k == EventKind::GitStatusOpen
            // || *k == EventKind::GitStatusApproved
            // || *k == EventKind::GitStatusClosed
            // || *k == EventKind::GitStatusDraft
            // || *k == EventKind::ProblemTracker
            // || *k == EventKind::Reporting
            // || *k == EventKind::Label
            // || *k == EventKind::CommunityPost
            // || *k == EventKind::CommunityPostApproval
            // || *k == EventKind::JobFeedback
            // || *k == EventKind::ZapGoal
                || *k == EventKind::ZapRequest
                || ((*k == EventKind::Zap) && enable_zap_receipts)
            // || *k == EventKind::Highlights
                || *k == EventKind::MuteList
            // || *k == EventKind::PinList
                || *k == EventKind::RelayList
            // || *k == EventKind::BookmarkList
            // || *k == EventKind::CommunityList
            // || *k == EventKind::PublicChatsList
            // || *k == EventKind::BlockedRelaysList
            // || *k == EventKind::SearchRelaysList
            // || *k == EventKind::UserGroups
            // || *k == EventKind::InterestsList
            // || *k == EventKind::UserEmojiList
                || (*k == EventKind::DmRelayList && direct_messages)
            // || *k == EventKind::FileStorageServerList
            // || *k == EventKind::WalletInfo
            // || *k == EventKind::LightningPubRpc
            // || *k == EventKind::Auth -- never subscribed to
            // || *k == EventKind::WalletRequest
            // || *k == EventKind::WalletResponse
            // || *k == EventKind::NostrConnect
            // || *k == EventKind::HttpAuth
                || *k == EventKind::FollowSets
            // || *k == EventKind::GenericSets
            // || *k == EventKind::RelaySets
            // || *k == EventKind::BookmarkSets
            // || *k == EventKind::CurationSets
            // || *k == EventKind::ProfileBadges
            // || *k == EventKind::BadgeDefinition
            // || *k == EventKind::InterestSets
            // || *k == EventKind::CreateUpdateStall
            // || *k == EventKind::CreateUpdateProduct
            // || *k == EventKind::MarketplaceUi
            // || *k == EventKind::ProductSoldAuction
                || ((*k == EventKind::LongFormContent) && show_long_form)
            // || *k == EventKind::DraftLongFormContent
            // || *k == EventKind::EmojiSets
            // || *k == EventKind::ReleaseArtifactSets
            // || *k == EventKind::AppSpecificData
            // || *k == EventKind::LiveEvent
            // || *k == EventKind::UserStatus
            // || *k == EventKind::ClassifiedListing
            // || *k == EventKind::DraftClassifiedListing
            // || *k == EventKind::RepositoryAnnouncement
            // || *k == EventKind::WikiArticle
            // || *k == EventKind::DateBasedCalendarEvent
            // || *k == EventKind::TimeBasedCalendarEvent
            // || *k == EventKind::Calendar
            // || *k == EventKind::CalendarEventRsvp
            // || *k == EventKind::HandlerRecommendation
            // || *k == EventKind::HandlerInformation
            // || *k == EventKind::CommunityDefinition
        })
        .collect()
}

pub fn feed_related_event_kinds(mut dms: bool) -> Vec<EventKind> {
    // Do not include DM kinds if identity is not unlocked
    if !GLOBALS.identity.is_unlocked() {
        dms = false;
    }

    enabled_event_kinds()
        .drain(..)
        .filter(|k| {
            k.is_feed_related()
                && (dms
                    || (*k != EventKind::EncryptedDirectMessage
                        && *k != EventKind::DmChat
                        && *k != EventKind::GiftWrap))
        })
        .collect()
}

pub fn feed_displayable_event_kinds(mut dms: bool) -> Vec<EventKind> {
    // Do not include DM kinds if identity is not unlocked
    if !GLOBALS.identity.is_unlocked() {
        dms = false;
    }
    enabled_event_kinds()
        .drain(..)
        .filter(|k| {
            k.is_feed_displayable()
                && (dms
                    || (*k != EventKind::EncryptedDirectMessage
                        && *k != EventKind::DmChat
                        && *k != EventKind::GiftWrap))
        })
        .collect()
}

pub fn feed_augment_event_kinds() -> Vec<EventKind> {
    enabled_event_kinds()
        .drain(..)
        .filter(|k| k.augments_feed_related())
        .collect()
}
