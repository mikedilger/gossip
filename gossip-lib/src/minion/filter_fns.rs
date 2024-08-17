use crate::filter_set::FeedRange;
use crate::globals::GLOBALS;
use nostr_types::{EventKind, Filter, IdHex, PublicKey, PublicKeyHex};

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
