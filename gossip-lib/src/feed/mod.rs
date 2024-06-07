mod feed_kind;
pub use feed_kind::FeedKind;

use crate::comms::{ToMinionMessage, ToMinionPayload, ToMinionPayloadDetail, ToOverlordMessage};
use crate::error::Error;
use crate::globals::GLOBALS;
use crate::people::PersonList;
use nostr_types::{Event, EventKind, EventReference, Filter, Id, PublicKeyHex, Unixtime};
use parking_lot::RwLock;
use std::collections::HashSet;
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
    current_feed_start: RwLock<Unixtime>,

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
            current_feed_start: RwLock::new(Unixtime::now().unwrap()),
            interval_ms: RwLock::new(10000), // Every 10 seconds, until we load from settings
            last_computed: RwLock::new(None),
            thread_parent: RwLock::new(None),
        }
    }

    /// This only looks further back in stored events, it doesn't deal with minion subscriptions.
    pub(crate) fn load_more(&self) -> Unixtime {
        let mut start = *self.current_feed_start.read();

        let kindstr = self.current_feed_kind.read().simple_string();
        let dur = if kindstr == "person" {
            60 * 60 * 24 * 15
        } else if kindstr == "inbox" {
            60 * 60 * 24 * 7
        } else {
            60 * 60 * 4
        };
        start = start - Duration::from_secs(dur);
        *self.current_feed_start.write() = start;
        start
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
        let kindstr = feed_kind.simple_string();

        // NOTE: do not clear the feed here, or the UI will get an empty feed momentarily
        // and the scroll bar "memory" will be reset to the top.  Let recompute rebuild
        // the feed (called down below)

        // Reset the feed start
        let dur = if kindstr == "person" {
            60 * 60 * 24 * 15
        } else if kindstr == "inbox" {
            60 * 60 * 24 * 7
        } else {
            60 * 60 * 4
        };
        *self.current_feed_start.write() = Unixtime::now().unwrap() - Duration::from_secs(dur);

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
                    .send(ToOverlordMessage::SetPersonFeed(*pubkey));
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

        let since: Unixtime = *self.current_feed_start.read();

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
                    Self::load_event_range(since, filter, with_replies, false, |_| true).await?
                };

                *self.current_feed_events.write() = events;
            }
            FeedKind::Inbox(indirect) => {
                if let Some(my_pubkey) = GLOBALS.identity.public_key() {
                    // Unfortunately it is expensive to find all events referencing
                    // any of my events, and we don't have such an index.
                    //
                    // so for now we rely on the fact that replies are supposed to
                    // 'p' tag the authors of people up the chain (see last paragraph
                    // of NIP-10)

                    let kinds_with_dms = feed_displayable_event_kinds(true);
                    let dismissed = GLOBALS.dismissed.read().await.clone();

                    let mut filter = Filter::new();
                    filter.kinds = kinds_with_dms.clone();
                    filter.add_author(&my_pubkey.into());
                    filter.since = Some(since);
                    let my_events: Vec<Event> =
                        GLOBALS.storage.find_events_by_filter(&filter, |_| true)?;
                    let my_event_ids: HashSet<Id> = my_events.iter().map(|e| e.id).collect();
                    let my_pubkeyhex: PublicKeyHex = my_pubkey.into();
                    let now = Unixtime::now().unwrap();

                    let inbox_events: Vec<Id> = GLOBALS
                        .storage
                        .find_tagged_events(
                            "p",
                            Some(my_pubkeyhex.as_str()),
                            |e| {
                                if e.created_at < since || e.created_at > now {
                                    return false;
                                }
                                if !kinds_with_dms.contains(&e.kind) {
                                    return false;
                                }
                                if dismissed.contains(&e.id) {
                                    return false;
                                }
                                if e.is_annotation() {
                                    return false;
                                }

                                // exclude if it's my own note
                                if e.pubkey == my_pubkey {
                                    return false;
                                }

                                if e.kind == EventKind::GiftWrap
                                    || e.kind == EventKind::EncryptedDirectMessage
                                {
                                    return true;
                                }

                                // Include if it directly replies to one of my events
                                match e.replies_to() {
                                    Some(EventReference::Id { id, .. }) => {
                                        if my_event_ids.contains(&id) {
                                            return true;
                                        }
                                    }
                                    Some(EventReference::Addr(ea)) => {
                                        if ea.author == my_pubkey {
                                            return true;
                                        }
                                    }
                                    None => (),
                                }

                                if indirect {
                                    // Include if it tags me
                                    e.people().iter().any(|(p, _, _)| *p == my_pubkey)
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

                    *self.current_feed_events.write() = inbox_events;
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

                let events = Self::load_event_range(since, filter, true, false, |_| true).await?;

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
        //let limit = GLOBALS.storage.read_setting_load_more_count() as usize;
        let dismissed = GLOBALS.dismissed.read().await.clone();

        let outer_screen =
            |e: &Event| basic_screen(e, include_replies, include_dms, &dismissed) && screen(e);

        //let mut before_filter = filter;
        //let mut after_filter = before_filter.clone();
        let mut after_filter = filter;

        //before_filter.until = Some(since);
        //before_filter.limit = Some(limit);

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
            /* Once we do anchor, we want to add this chain
               .chain(
               GLOBALS
               .storage
               .find_events_by_filter(&before_filter, outer_screen)?
               .iter()
               .map(|e| e.id),
            )
               */
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
