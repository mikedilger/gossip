use crate::dm_channel::DmChannel;
use crate::globals::GLOBALS;
use nostr_types::{EventKind, Filter, IdHex, PublicKey, PublicKeyHex, Unixtime};

pub fn general_feed(
    authors: &[PublicKey],
    since: Unixtime,
    until: Option<Unixtime>,
) -> Vec<Filter> {
    let mut filters: Vec<Filter> = Vec::new();

    if authors.is_empty() {
        return vec![];
    }

    let pkp: Vec<PublicKeyHex> = authors.iter().map(|pk| pk.into()).collect();

    let event_kinds = crate::feed::feed_related_event_kinds(false);

    // feed related by people followed
    filters.push(Filter {
        authors: pkp,
        kinds: event_kinds.clone(),
        since: Some(since),
        until,
        ..Default::default()
    });

    filters
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

pub fn inbox(since: Unixtime, until: Option<Unixtime>, spamsafe: bool) -> Vec<Filter> {
    let mut filters: Vec<Filter> = Vec::new();

    // GiftWrap lookback needs to be one week further back
    // FIXME: this depends on how far other clients backdate.
    let giftwrap_since = Unixtime(since.0 - 60 * 60 * 24 * 7);
    let giftwrap_until = until.map(|u| Unixtime(u.0 - 60 * 60 * 24 * 7));

    // Allow all feed related event kinds (including DMs)
    let mut event_kinds = crate::feed::feed_related_event_kinds(true);
    event_kinds.retain(|f| *f != EventKind::GiftWrap); // gift wrap has special filter

    if let Some(pubkey) = GLOBALS.identity.public_key() {
        // Any mentions of me
        // (but not in peoples contact lists, for example)

        let pkh: PublicKeyHex = pubkey.into();

        let filter = {
            let mut filter = Filter {
                kinds: event_kinds,
                since: Some(since),
                until,
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

        // Giftwrap specially looks back further
        // Giftwraps cannot be filtered by author so we have to take them regardless
        // of the spamsafe designation of the relay.
        let filter = {
            let mut filter = Filter {
                kinds: vec![EventKind::GiftWrap],
                since: Some(giftwrap_since),
                until: giftwrap_until,
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

pub fn outbox(since: Unixtime) -> Vec<Filter> {
    if let Some(pubkey) = GLOBALS.identity.public_key() {
        let pkh: PublicKeyHex = pubkey.into();
        let giftwrap_since = Unixtime(since.0 - 60 * 60 * 24 * 7);

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
            Filter {
                authors: vec![pkh.clone()],
                kinds: vec![EventKind::GiftWrap],
                since: Some(giftwrap_since),
                ..Default::default()
            },
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

pub fn person_feed(pubkey: PublicKey, since: Unixtime, until: Option<Unixtime>) -> Vec<Filter> {
    // Allow all feed related event kinds (excluding DMs)
    let event_kinds = crate::feed::feed_displayable_event_kinds(false);

    vec![Filter {
        authors: vec![pubkey.into()],
        kinds: event_kinds,
        since: Some(since),
        until,
        ..Default::default()
    }]
}

pub fn thread(main: IdHex, ancestors: &[IdHex], spamsafe: bool) -> Vec<Filter> {
    let mut filters: Vec<Filter> = Vec::new();

    if !ancestors.is_empty() {
        // We allow spammy ancestors since a descendant is sought, so spamsafe
        // isn't relevant to these ancestor filters

        // Get ancestors we know of so far
        filters.push(Filter {
            ids: ancestors.to_vec(),
            ..Default::default()
        });

        // Get reactions to ancestors, but not replies
        let kinds = crate::feed::feed_augment_event_kinds();
        let filter = {
            let mut filter = Filter {
                kinds,
                ..Default::default()
            };
            let values = ancestors.iter().map(|id| id.to_string()).collect();
            filter.set_tag_values('e', values);
            filter
        };
        filters.push(filter);
    }

    // Allow all feed related event kinds (excluding DMs)
    let event_kinds = crate::feed::feed_related_event_kinds(false);

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
        authors,
        kinds: vec![EventKind::EncryptedDirectMessage],
        ..Default::default()
    };
    // tagging the user
    filter.set_tag_values('p', vec![pkh.to_string()]);

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
