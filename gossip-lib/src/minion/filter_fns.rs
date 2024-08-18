use crate::filter_set::FeedRange;
use nostr_types::{Filter, PublicKey};

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
