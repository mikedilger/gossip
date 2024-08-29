use crate::dm_channel::DmChannel;
use crate::globals::GLOBALS;
use nostr_types::{EventKind, Filter, IdHex, NAddr, PublicKey, PublicKeyHex, Tag, Unixtime};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub enum FeedRange {
    // Long-term subscription for anything after the given time
    After { since: Unixtime },

    // Short-term subscription for up to limit events preceding the until time
    ChunkBefore { until: Unixtime, limit: usize },
}

impl FeedRange {
    pub fn since_until_limit(&self) -> (Option<Unixtime>, Option<Unixtime>, Option<usize>) {
        match *self {
            FeedRange::After { since } => (Some(since), None, None),
            FeedRange::ChunkBefore { until, limit } => (None, Some(until), Some(limit)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FilterSet {
    Augments(Vec<IdHex>),
    Config,
    Discover(Vec<PublicKey>),
    DmChannel(DmChannel),
    GeneralFeedFuture {
        pubkeys: Vec<PublicKey>,
        anchor: Unixtime,
    },
    GeneralFeedChunk {
        pubkeys: Vec<PublicKey>,
        anchor: Unixtime,
    },
    Giftwraps(FeedRange),
    GlobalFeedFuture(Unixtime),
    GlobalFeedChunk(Unixtime),
    InboxFeedFuture(Unixtime),
    InboxFeedChunk(Unixtime),
    Metadata(Vec<PublicKey>),
    Nip46,
    PersonFeedFuture {
        pubkey: PublicKey,
        anchor: Unixtime,
    },
    PersonFeedChunk {
        pubkey: PublicKey,
        anchor: Unixtime,
    },
    RepliesToId(IdHex),
    RepliesToAddr(NAddr),
}

impl FilterSet {
    pub fn temporary(&self) -> bool {
        match self {
            FilterSet::Augments(_) => true,
            FilterSet::Config => false,
            FilterSet::Discover(_) => true,
            FilterSet::DmChannel(_) => false,
            FilterSet::GeneralFeedFuture { .. } => false,
            FilterSet::GeneralFeedChunk { .. } => true,
            FilterSet::Giftwraps(_) => false,
            FilterSet::GlobalFeedFuture(_) => false,
            FilterSet::GlobalFeedChunk(_) => true,
            FilterSet::InboxFeedFuture(_) => false,
            FilterSet::InboxFeedChunk(_) => true,
            FilterSet::Metadata(_) => true,
            FilterSet::Nip46 => false,
            FilterSet::PersonFeedFuture { .. } => false,
            FilterSet::PersonFeedChunk { .. } => true,
            FilterSet::RepliesToId(_) => false,
            FilterSet::RepliesToAddr(_) => false,
        }
    }

    pub fn can_have_duplicates(&self) -> bool {
        match self {
            FilterSet::GeneralFeedChunk { .. } => true,
            FilterSet::GlobalFeedChunk(_) => true,
            FilterSet::InboxFeedChunk(_) => true,
            FilterSet::PersonFeedChunk { .. } => true,
            _ => false,
        }
    }

    pub fn is_loading_more(&self) -> bool {
        match self {
            FilterSet::GeneralFeedChunk { .. } => true,
            FilterSet::GlobalFeedChunk(_) => true,
            FilterSet::InboxFeedChunk(_) => true,
            FilterSet::PersonFeedChunk { .. } => true,
            _ => false,
        }
    }

    pub fn inner_handle(&self) -> &'static str {
        match self {
            FilterSet::Augments(_) => "augments",
            FilterSet::Config => "config_feed",
            FilterSet::Discover(_) => "discover_feed",
            FilterSet::DmChannel(_) => "dm_channel",
            FilterSet::GeneralFeedFuture { .. } => "general_feed",
            FilterSet::GeneralFeedChunk { .. } => "general_feed_chunk",
            FilterSet::Giftwraps(_) => "giftwraps",
            FilterSet::GlobalFeedFuture(_) => "global_feed",
            FilterSet::GlobalFeedChunk(_) => "global_feed_chunk",
            FilterSet::InboxFeedFuture(_) => "inbox_feed",
            FilterSet::InboxFeedChunk(_) => "inbox_feed_chunk",
            FilterSet::Metadata(_) => "subscribe_metadata",
            FilterSet::Nip46 => "nip46",
            FilterSet::PersonFeedFuture { .. } => "person_feed",
            FilterSet::PersonFeedChunk { .. } => "person_feed_chunk",
            FilterSet::RepliesToId(_) => "id_replies",
            FilterSet::RepliesToAddr(_) => "addr_replies",
        }
    }

    pub fn handle(&self, job_id: u64) -> String {
        let mut handle = self.inner_handle().to_owned();
        if self.temporary() {
            handle = format!("temp_{}", handle);
        }
        if self.can_have_duplicates() {
            handle = format!("{}_{}", handle, job_id)
        }
        handle
    }

    pub async fn filters(&self, spamsafe: bool) -> Vec<Filter> {
        let mut filters: Vec<Filter> = Vec::new();

        match self {
            FilterSet::Augments(ids) => {
                let event_kinds = crate::feed::feed_augment_event_kinds();
                let filter = {
                    let mut filter = Filter {
                        kinds: event_kinds,
                        ..Default::default()
                    };
                    filter.set_tag_values('e', ids.iter().map(|id| id.to_string()).collect());
                    filter
                };
                filters.push(filter);
            }
            FilterSet::Config => {
                let since = Unixtime::now() - Duration::from_secs(60 * 60 * 24 * 15);
                if let Some(pubkey) = GLOBALS.identity.public_key().await {
                    let pkh: PublicKeyHex = pubkey.into();
                    if GLOBALS.identity.is_unlocked().await {
                        // GiftWraps to me, recent only
                        let giftwrap_since = Unixtime(since.0 - 60 * 60 * 24 * 7);
                        let giftwrap_filter = {
                            let mut f = Filter {
                                kinds: vec![EventKind::GiftWrap],
                                since: Some(giftwrap_since),
                                ..Default::default()
                            };
                            f.set_tag_values('p', vec![pkh.to_string()]);
                            f
                        };
                        filters.push(giftwrap_filter);
                    }

                    // Actual config stuff
                    filters.push(Filter {
                        authors: vec![pkh.clone()],
                        kinds: vec![
                            EventKind::Metadata,
                            //EventKind::RecommendRelay,
                            EventKind::ContactList,
                            EventKind::MuteList,
                            EventKind::FollowSets,
                            EventKind::RelayList,
                            EventKind::DmRelayList,
                            EventKind::BookmarkList,
                        ],
                        // these are all replaceable, no since required
                        ..Default::default()
                    });

                    // Events I posted recently, including feed_displayable and
                    //  augments (deletions, reactions, timestamp, label,reporting, and zap)
                    filters.push(Filter {
                        authors: vec![pkh],
                        kinds: crate::feed::feed_related_event_kinds(false), // not DMs
                        since: Some(since),
                        ..Default::default()
                    });
                }
            }
            FilterSet::Discover(pubkeys) => {
                let pkp: Vec<PublicKeyHex> = pubkeys.iter().map(|pk| pk.into()).collect();
                filters.push(Filter {
                    authors: pkp,
                    kinds: vec![EventKind::RelayList, EventKind::DmRelayList],
                    // these are all replaceable, no since required
                    ..Default::default()
                });
            }
            FilterSet::DmChannel(channel) => {
                let pubkey = match GLOBALS.identity.public_key().await {
                    Some(pk) => pk,
                    None => return vec![],
                };
                let pkh: PublicKeyHex = pubkey.into();

                // note: giftwraps can't be subscribed by channel. they are subscribed more
                // globally, and have to be limited to recent ones.

                let mut authors: Vec<PublicKeyHex> =
                    channel.keys().iter().map(|k| k.into()).collect();
                authors.push(pkh.clone());

                let mut filter = Filter {
                    authors: authors.clone(),
                    kinds: vec![EventKind::EncryptedDirectMessage],
                    ..Default::default()
                };
                // tagging the user
                filter.set_tag_values('p', authors.iter().map(|x| x.as_str().to_owned()).collect());
                filters.push(filter);
            }
            FilterSet::GeneralFeedFuture { pubkeys, anchor } => {
                if pubkeys.is_empty() {
                    return vec![];
                }

                let pkp: Vec<PublicKeyHex> = pubkeys.iter().map(|pk| pk.into()).collect();

                // Do not load feed related event kinds, or the limit will be wrong
                let event_kinds = crate::feed::feed_displayable_event_kinds(false);

                let range = FeedRange::After { since: *anchor };
                let (since, until, limit) = range.since_until_limit();
                filters.push(Filter {
                    authors: pkp,
                    kinds: event_kinds,
                    since,
                    until,
                    limit,
                    ..Default::default()
                });
            }
            FilterSet::GeneralFeedChunk { pubkeys, anchor } => {
                if pubkeys.is_empty() {
                    return vec![];
                }

                let pkp: Vec<PublicKeyHex> = pubkeys.iter().map(|pk| pk.into()).collect();

                // Do not load feed related event kinds, or the limit will be wrong
                let event_kinds = crate::feed::feed_displayable_event_kinds(false);

                let limit = GLOBALS.db().read_setting_load_more_count() as usize;
                let range = FeedRange::ChunkBefore {
                    until: *anchor,
                    limit,
                };
                let (since, until, limit) = range.since_until_limit();
                filters.push(Filter {
                    authors: pkp,
                    kinds: event_kinds,
                    since,
                    until,
                    limit,
                    ..Default::default()
                });
            }
            FilterSet::Giftwraps(range) => {
                let (since, until, limit) = range.since_until_limit();
                if let Some(pubkey) = GLOBALS.identity.public_key().await {
                    let pkh: PublicKeyHex = pubkey.into();

                    // Giftwraps cannot be filtered by author so we have to take them regardless
                    // of the spamsafe designation of the relay.
                    //
                    // Sure, the TOTAL number of giftwraps being the limit will be MORE than we need,
                    // but since giftwraps get backdated, this is probably a good thing.
                    let filter = {
                        let mut filter = Filter {
                            kinds: vec![EventKind::GiftWrap],
                            // giftwraps may be dated 1 week in the past:
                            since: since.map(|u| Unixtime(*u - (3600 * 24 * 7))),
                            until,
                            limit,
                            ..Default::default()
                        };
                        let values = vec![pkh.to_string()];
                        filter.set_tag_values('p', values);
                        filter
                    };
                    filters.push(filter);
                }
            }
            FilterSet::GlobalFeedFuture(anchor) => {
                // Allow all feed related event kinds (excluding DMs)
                // Do not load feed related or the limit will be wrong
                let event_kinds = crate::feed::feed_displayable_event_kinds(false);

                let range = FeedRange::After { since: *anchor };
                let (since, until, limit) = range.since_until_limit();
                filters.push(Filter {
                    kinds: event_kinds.clone(),
                    since,
                    until,
                    limit,
                    ..Default::default()
                });
            }
            FilterSet::GlobalFeedChunk(anchor) => {
                // Allow all feed related event kinds (excluding DMs)
                // Do not load feed related or the limit will be wrong
                let event_kinds = crate::feed::feed_displayable_event_kinds(false);

                let limit = GLOBALS.db().read_setting_load_more_count() as usize;
                let range = FeedRange::ChunkBefore {
                    until: *anchor,
                    limit,
                };
                let (since, until, limit) = range.since_until_limit();
                filters.push(Filter {
                    kinds: event_kinds,
                    since,
                    until,
                    limit,
                    ..Default::default()
                });
            }
            FilterSet::InboxFeedFuture(anchor) => {
                if let Some(pubkey) = GLOBALS.identity.public_key().await {
                    let mut filter = Self::inbox_base_filter(pubkey, spamsafe);

                    let range = FeedRange::After { since: *anchor };
                    let (since, until, limit) = range.since_until_limit();
                    filter.since = since;
                    filter.until = until;
                    filter.limit = limit;
                    filters.push(filter);
                }
            }
            FilterSet::InboxFeedChunk(anchor) => {
                if let Some(pubkey) = GLOBALS.identity.public_key().await {
                    let mut filter = Self::inbox_base_filter(pubkey, spamsafe);

                    let limit = GLOBALS.db().read_setting_load_more_count() as usize;
                    let range = FeedRange::ChunkBefore {
                        until: *anchor,
                        limit,
                    };
                    let (since, until, limit) = range.since_until_limit();
                    filter.since = since;
                    filter.until = until;
                    filter.limit = limit;
                    filters.push(filter.clone());
                }
            }
            FilterSet::Metadata(pubkeys) => {
                let pkhp: Vec<PublicKeyHex> = pubkeys.iter().map(|pk| pk.into()).collect();
                filters.push(Filter {
                    authors: pkhp,
                    kinds: vec![
                        EventKind::Metadata,
                        EventKind::RelayList,
                        EventKind::DmRelayList,
                    ],
                    // FIXME: we could probably get a since-last-fetched-their-metadata here.
                    //        but relays should just return the latest of these.
                    ..Default::default()
                });
            }
            FilterSet::Nip46 => {
                let pubkey = match GLOBALS.identity.public_key().await {
                    Some(pk) => pk,
                    None => return vec![],
                };
                let pkh: PublicKeyHex = pubkey.into();

                let mut filter = Filter {
                    kinds: vec![EventKind::NostrConnect],
                    ..Default::default()
                };
                filter.set_tag_values('p', vec![pkh.to_string()]);
                filters.push(filter);
            }
            FilterSet::PersonFeedFuture { pubkey, anchor } => {
                // Allow all feed related event kinds (excluding DMs)
                // Do not load feed related or the limit will be wrong
                let event_kinds = crate::feed::feed_displayable_event_kinds(false);

                let range = FeedRange::After { since: *anchor };
                let (since, until, limit) = range.since_until_limit();
                filters.push(Filter {
                    authors: vec![pubkey.into()],
                    kinds: event_kinds,
                    since,
                    until,
                    limit,
                    ..Default::default()
                });
            }
            FilterSet::PersonFeedChunk { pubkey, anchor } => {
                // Allow all feed related event kinds (excluding DMs)
                // Do not load feed related or the limit will be wrong
                let event_kinds = crate::feed::feed_displayable_event_kinds(false);

                let limit = GLOBALS.db().read_setting_load_more_count() as usize;
                let range = FeedRange::ChunkBefore {
                    until: *anchor,
                    limit,
                };

                let (since, until, limit) = range.since_until_limit();
                filters.push(Filter {
                    authors: vec![pubkey.into()],
                    kinds: event_kinds,
                    since,
                    until,
                    limit,
                    ..Default::default()
                });
            }
            FilterSet::RepliesToId(id) => {
                // Allow all feed related event kinds (excluding DMs)
                // (related because we want deletion events, and may as well get likes and zaps too)
                let event_kinds = crate::feed::feed_related_event_kinds(false);
                let filter = {
                    let mut filter = Filter {
                        kinds: event_kinds,
                        ..Default::default()
                    };
                    let values = vec![id.to_string()];
                    filter.set_tag_values('e', values);

                    // Spam prevention:
                    if !spamsafe && GLOBALS.db().read_setting_avoid_spam_on_unsafe_relays() {
                        filter.authors = GLOBALS
                            .people
                            .get_subscribed_pubkeys()
                            .drain(..)
                            .map(|pk| pk.into())
                            .collect();
                    }

                    filter
                };
                filters.push(filter);
            }
            FilterSet::RepliesToAddr(addr) => {
                // Allow all feed related event kinds (excluding DMs)
                // (related because we want deletion events, and may as well get likes and zaps too)
                let event_kinds = crate::feed::feed_related_event_kinds(false);
                let filter = {
                    let mut filter = Filter {
                        kinds: event_kinds,
                        ..Default::default()
                    };
                    let a_tag = Tag::new_address(addr, None);
                    filter.set_tag_values('a', vec![a_tag.value().to_owned()]);

                    // Spam prevention:
                    if !spamsafe && GLOBALS.db().read_setting_avoid_spam_on_unsafe_relays() {
                        filter.authors = GLOBALS
                            .people
                            .get_subscribed_pubkeys()
                            .drain(..)
                            .map(|pk| pk.into())
                            .collect();
                    }

                    filter
                };
                filters.push(filter);
            }
        }

        filters
    }

    fn inbox_base_filter(pubkey: PublicKey, spamsafe: bool) -> Filter {
        // Allow all feed displayable event kinds (including DMs)
        let mut event_kinds = crate::feed::feed_displayable_event_kinds(true);
        event_kinds.retain(|f| *f != EventKind::GiftWrap); // gift wrap is not included here

        // Any mentions of me (but not in peoples contact lists, for example)
        let pkh: PublicKeyHex = pubkey.into();

        let mut filter = Filter {
            kinds: event_kinds,
            ..Default::default()
        };

        let values = vec![pkh.to_string()];
        filter.set_tag_values('p', values);

        // Spam prevention:
        if !spamsafe && GLOBALS.db().read_setting_avoid_spam_on_unsafe_relays() {
            // As the relay is not spam safe, only take mentions from followers
            filter.authors = GLOBALS
                .people
                .get_subscribed_pubkeys()
                .drain(..)
                .map(|pk| pk.into())
                .collect();
        }

        filter
    }
}
