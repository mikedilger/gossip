use crate::dm_channel::DmChannel;
use crate::filter_set::FeedRange;
use crate::globals::GLOBALS;
use nostr_types::{EventKind, Filter, IdHex, NAddr, PublicKey, PublicKeyHex, Tag};

pub fn inbox_feed(spamsafe: bool, range: FeedRange) -> Vec<Filter> {
    let mut filters: Vec<Filter> = Vec::new();

    // Allow all feed displayable event kinds (including DMs)
    let mut event_kinds = crate::feed::feed_displayable_event_kinds(true);
    event_kinds.retain(|f| *f != EventKind::GiftWrap); // gift wrap is not included here

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
    }

    filters
}

pub fn person_feed(pubkey: PublicKey, range: FeedRange) -> Vec<Filter> {
    // Allow all feed related event kinds (excluding DMs)
    // Do not load feed related or the limit will be wrong
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

pub fn global_feed(range: FeedRange) -> Vec<Filter> {
    // Allow all feed related event kinds (excluding DMs)
    // Do not load feed related or the limit will be wrong
    let event_kinds = crate::feed::feed_displayable_event_kinds(false);

    let (since, until, limit) = range.since_until_limit();

    vec![Filter {
        kinds: event_kinds,
        since,
        until,
        limit,
        ..Default::default()
    }]
}

pub fn replies(main: IdHex, spamsafe: bool) -> Vec<Filter> {
    let mut filters: Vec<Filter> = Vec::new();

    // Allow all feed related event kinds (excluding DMs)
    // (related because we want deletion events, and may as well get likes and zaps too)
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

pub fn replies_to_eaddr(ea: &NAddr, spamsafe: bool) -> Vec<Filter> {
    let mut filters: Vec<Filter> = Vec::new();

    // Allow all feed related event kinds (excluding DMs)
    // (related because we want deletion events, and may as well get likes and zaps too)
    let event_kinds = crate::feed::feed_related_event_kinds(false);

    let filter = {
        let mut filter = Filter {
            kinds: event_kinds,
            ..Default::default()
        };
        let a_tag = Tag::new_address(ea, None);
        filter.set_tag_values('a', vec![a_tag.value().to_owned()]);

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
        kinds: vec![
            EventKind::Metadata,
            EventKind::RelayList,
            EventKind::DmRelayList,
        ],
        // FIXME: we could probably get a since-last-fetched-their-metadata here.
        //        but relays should just return the latest of these.
        ..Default::default()
    }]
}
