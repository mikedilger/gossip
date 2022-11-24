use crate::db::{DbPerson, DbPersonRelay, DbRelay};
use crate::Error;
use nostr_proto::{
    EventKind, Filters, PublicKeyHex, Unixtime, Url,
};
use std::collections::HashMap;

mod websocket_handler;
pub use websocket_handler::WebsocketHandler;

/// This function computes which relays we need to follow and what filters
/// they should have, only for startup, based on what is in the database.
pub async fn load_initial_relay_filters() -> Result<HashMap<Url, Filters>, Error> {

    // Start collecting filters per-relay
    let mut per_relay_filters: HashMap<Url, Filters> = HashMap::new();

    // Build a hashmap of relays that we know
    let mut relays = DbRelay::fetch(None).await?;
    let mut relaymap: HashMap<String, DbRelay> = HashMap::new();
    for relay in relays.drain(..) {
        relaymap.insert(relay.url.clone(), relay);
    }

    // Load all the people we are following
    let people = DbPerson::fetch(Some("followed=1")).await?;

    // Remember people for which we have no relay information
    let mut orphan_pubkeys: Vec<PublicKeyHex> = Vec::new();

    for person in people.iter() {

        let public_key: PublicKeyHex = PublicKeyHex(person.pubkey.0.clone());

        // Load which relays they use
        let person_relays =
            DbPersonRelay::fetch(Some(&format!("person='{}'", person.pubkey))).await?;

        // Get the highest ranked relay that they use
        let best_relay: Option<DbRelay> = person_relays.iter()
            .map_while(|pr| relaymap.get(&pr.relay))
            .fold(None, |current, candidate| {
                if let Some(cur) = current {
                    if cur.rank >= candidate.rank { Some(cur) }
                    else { Some(candidate.clone()) }
                } else {
                    Some(candidate.clone())
                }
            });

        if let Some(relay) = best_relay {
            let url: Url = Url(relay.url.clone());
            let entry = per_relay_filters.entry(url).or_default();
            entry.add_author(&public_key, None);
        } else {
            // if they have no relay, mark them as an orphan
            orphan_pubkeys.push(person.pubkey.clone())
        }
    }

    // Listen to orphans on all relays we are already listening on
    for orphan in orphan_pubkeys.iter() {
        for (_url, filters) in per_relay_filters.iter_mut() {
            let public_key = orphan.clone();
            filters.add_author(&public_key, None);
        }
    }

    // Update all the filters
    {
        for (url, filters) in per_relay_filters.iter_mut() {

            log::debug!("We will listen to {}, {:?}", &url, filters.authors);

            // Listen to these six kinds of events
            filters.add_event_kind(EventKind::Metadata);
            filters.add_event_kind(EventKind::TextNote);
            filters.add_event_kind(EventKind::RecommendRelay);
            filters.add_event_kind(EventKind::ContactList);
            filters.add_event_kind(EventKind::EventDeletion);
            filters.add_event_kind(EventKind::Reaction);

            // On startup, only pick up events in the last 12 hours
            let mut start = Unixtime::now().unwrap();
            start.0 -= 43200;

            // LETS BE NICE and not get messages from too far back
            filters.since = Some(start);

            // TODO - check the database for which events we are up to.
        }
    }

    for (url, filters) in per_relay_filters.iter() {
        log::info!("WILL WATCH {} WITH {:?}", &url, filters);
    }

    Ok(per_relay_filters)
}
