use crate::Error;
use crate::db::{DbPerson, DbPersonRelay};
use nostr_proto::{Url, EventKind, Filters, PublicKey, Unixtime};
use std::collections::HashMap;

/// This function computes which relays we need to follow and what filters
/// they should have, only for startup, based on what is in the database.
async fn load_initial_relay_filters() -> Result<HashMap<Url, Filters>, Error> {

    let mut hashmap: HashMap<Url, Filters> = HashMap::new();

    // Load all the people we are following
    let people = crate::db::DbPerson::fetch(Some("following=1")).await?;
    for person in people.iter() {

        let public_key: PublicKey = PublicKey::try_from_hex_string(&person.public_key)?;

        // Load which relays they use
        let person_relays = crate::db::DbPersonRelay::fetch(
            Some(&format!("person='{}'", person.public_key))
        ).await?;

        for person_relay in person_relays.iter() {
            let url: Url = Url(person_relay.relay.clone());

            let entry = hashmap.entry(url)
                .or_insert(Default::default());

            entry.add_author(&public_key, None);
        }

        // If they have no relay, we will handle them next loop
    }

    // Update all the filters
    {
        for (_url, relay_data) in hashmap.iter_mut() {
            // Listen to these six kinds of events
            relay_data.add_event_kind(EventKind::Metadata);
            relay_data.add_event_kind(EventKind::TextNote);
            relay_data.add_event_kind(EventKind::RecommendRelay);
            relay_data.add_event_kind(EventKind::ContactList);
            relay_data.add_event_kind(EventKind::EventDeletion);
            relay_data.add_event_kind(EventKind::Reaction);

            // On startup, only pick up events in the last 12 hours
            let mut start = Unixtime::now().unwrap();
            start.0 = start.0 - 43200;

            // LETS BE NICE and not get messages from too far back
            relay_data.since = Some(start);

            // TODO - check the database for which events we are up to.
        }
    }

    Ok(hashmap)
}
