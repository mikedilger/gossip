use crate::dm_channel::DmChannel;
use crate::globals::GLOBALS;
use nostr_types::{EventKind, Filter, IdHex, PublicKey, PublicKeyHex, Unixtime};

pub enum FeedRange {
    // Long-term subscription for anything after the given time
    After {
        since: Unixtime,
    },

    // Short-term subscription for up to limit events preceding the until time
    #[allow(dead_code)]
    Before {
        until: Unixtime,
        limit: usize,
    },

    // Original chunking method
    OriginalChunk {
        since: Unixtime,
        until: Unixtime,
    },
}

impl FeedRange {
    pub fn since_until_limit(&self) -> (Option<Unixtime>, Option<Unixtime>, Option<usize>) {
        match *self {
            FeedRange::After { since } => (Some(since), None, None),
            FeedRange::Before { until, limit } => (None, Some(until), Some(limit)),
            FeedRange::OriginalChunk { since, until } => (Some(since), Some(until), None),
        }
    }
}

pub fn general_feed(authors: &[PublicKey], range: FeedRange) -> Vec<Filter> {
    let mut filters: Vec<Filter> = Vec::new();

    if authors.is_empty() {
        return vec![];
    }

    let pkp: Vec<PublicKeyHex> = authors.iter().map(|pk| pk.into()).collect();

    let event_kinds = crate::feed::feed_related_event_kinds(false);

    let (since, until, limit) = range.since_until_limit();

    // feed related by people followed
    filters.push(Filter {
        authors: pkp,
        kinds: event_kinds.clone(),
        since,
        until,
        limit,
        ..Default::default()
    });

    filters
}

pub fn inbox_feed(spamsafe: bool, range: FeedRange) -> Vec<Filter> {
    let mut filters: Vec<Filter> = Vec::new();

    // Allow all feed related event kinds (including DMs)
    let mut event_kinds = crate::feed::feed_related_event_kinds(true);
    event_kinds.retain(|f| *f != EventKind::GiftWrap); // gift wrap has special filter

    let (since, until, limit) = range.since_until_limit();

    if let Some(pubkey) = GLOBALS.identity.public_key() {
        // Any mentions of me
        // (but not in peoples contact lists, for example)

        let pkh: PublicKeyHex = pubkey.into();

        let filter = {
            let mut filter = Filter {
                kinds: event_kinds,
                since,
                until,
                limit,
                ..Default::default()
            };
            let values = vec![pkh.to_string()];
            filter.set_tag_values('p', values);

            // Spam prevention:
            if !spamsafe && GLOBALS.storage.read_setting_avoid_spam_on_unsafe_relays() {
                // As the relay is not spam safe, only take mentions from followers
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

        // Giftwraps cannot be filtered by author so we have to take them regardless
        // of the spamsafe designation of the relay.
        //
        // Sure, the TOTAL number of giftwraps being the limit will be MORE than we need,
        // but since giftwraps get backdated, this is probably a good thing.
        let filter = {
            let mut filter = Filter {
                kinds: vec![EventKind::GiftWrap],
                since,
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

    filters
}

pub fn person_feed(pubkey: PublicKey, range: FeedRange) -> Vec<Filter> {
    // Allow all feed related event kinds (excluding DMs)
    let event_kinds = crate::feed::feed_displayable_event_kinds(false);

    let (since, until, limit) = range.since_until_limit();

    vec![Filter {
        authors: vec![pubkey.into()],
        kinds: event_kinds,
        since,
        until,
        limit,
        ..Default::default()
    }]
}

pub fn augments(ids: &[IdHex]) -> Vec<Filter> {
    let event_kinds = crate::feed::feed_augment_event_kinds();

    let filter = {
        let mut filter = Filter {
            kinds: event_kinds,
            ..Default::default()
        };
        filter.set_tag_values('e', ids.iter().map(|id| id.to_string()).collect());
        filter
    };

    vec![filter]
}

pub fn config(since: Unixtime) -> Vec<Filter> {
    if let Some(pubkey) = GLOBALS.identity.public_key() {
        let pkh: PublicKeyHex = pubkey.into();
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

        // Read back in things that we wrote out to our write relays
        // that we need
        vec![
            // Actual config stuff
            Filter {
                authors: vec![pkh.clone()],
                kinds: vec![
                    EventKind::Metadata,
                    //EventKind::RecommendRelay,
                    EventKind::ContactList,
                    EventKind::MuteList,
                    EventKind::FollowSets,
                    EventKind::RelayList,
                ],
                // these are all replaceable, no since required
                ..Default::default()
            },
            // GiftWraps to me, recent only
            giftwrap_filter,
            // Events I posted recently, including feed_displayable and
            //  augments (deletions, reactions, timestamp, label,reporting, and zap)
            Filter {
                authors: vec![pkh],
                kinds: crate::feed::feed_related_event_kinds(false), // not DMs
                since: Some(since),
                ..Default::default()
            },
        ]
    } else {
        vec![]
    }
}

// This FORCES the fetch of relay lists without checking if we need them.
// See also relay_lists() which checks if they are needed first.
pub fn discover(pubkeys: &[PublicKey]) -> Vec<Filter> {
    let pkp: Vec<PublicKeyHex> = pubkeys.iter().map(|pk| pk.into()).collect();
    vec![Filter {
        authors: pkp,
        kinds: vec![EventKind::RelayList],
        // these are all replaceable, no since required
        ..Default::default()
    }]
}

// ancestors can be done with FetchEvent, FetchEventAddr

pub fn replies(main: IdHex, spamsafe: bool) -> Vec<Filter> {
    let mut filters: Vec<Filter> = Vec::new();

    // Allow all feed related event kinds (excluding DMs)
    let event_kinds = crate::feed::feed_displayable_event_kinds(false);

    let filter = {
        let mut filter = Filter {
            kinds: event_kinds,
            ..Default::default()
        };
        let values = vec![main.to_string()];
        filter.set_tag_values('e', values);

        // Spam prevention:
        if !spamsafe && GLOBALS.storage.read_setting_avoid_spam_on_unsafe_relays() {
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

    filters
}

pub fn dm_channel(dmchannel: DmChannel) -> Vec<Filter> {
    let pubkey = match GLOBALS.identity.public_key() {
        Some(pk) => pk,
        None => return vec![],
    };
    let pkh: PublicKeyHex = pubkey.into();

    // note: giftwraps can't be subscribed by channel. they are subscribed more
    // globally, and have to be limited to recent ones.

    let mut authors: Vec<PublicKeyHex> = dmchannel.keys().iter().map(|k| k.into()).collect();
    authors.push(pkh.clone());

    let mut filter = Filter {
        authors: authors.clone(),
        kinds: vec![EventKind::EncryptedDirectMessage],
        ..Default::default()
    };
    // tagging the user
    filter.set_tag_values('p', authors.iter().map(|x| x.as_str().to_owned()).collect());

    vec![filter]
}

pub fn nip46() -> Vec<Filter> {
    let pubkey = match GLOBALS.identity.public_key() {
        Some(pk) => pk,
        None => return vec![],
    };
    let pkh: PublicKeyHex = pubkey.into();

    let mut filter = Filter {
        kinds: vec![EventKind::NostrConnect],
        ..Default::default()
    };
    filter.set_tag_values('p', vec![pkh.to_string()]);

    vec![filter]
}

pub fn metadata(pubkeys: &[PublicKey]) -> Vec<Filter> {
    let pkhp: Vec<PublicKeyHex> = pubkeys.iter().map(|pk| pk.into()).collect();

    vec![Filter {
        authors: pkhp,
        kinds: vec![EventKind::Metadata, EventKind::RelayList],
        // FIXME: we could probably get a since-last-fetched-their-metadata here.
        //        but relays should just return the lastest of these.
        ..Default::default()
    }]
}
