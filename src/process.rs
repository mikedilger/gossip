use crate::comms::ToOverlordMessage;
use crate::db::{
    DbEvent, DbEventHashtag, DbEventRelationship, DbEventRelay, DbEventTag, DbPersonRelay, DbRelay,
};
use crate::error::Error;
use crate::globals::{Globals, GLOBALS};
use crate::relationship::Relationship;
use nostr_types::{
    Event, EventKind, Metadata, PublicKeyHex, RelayUrl, SimpleRelayList, Tag, Unixtime,
};
use std::sync::atomic::Ordering;

// This processes a new event, saving the results into the database
// and also populating the GLOBALS maps.
pub async fn process_new_event(
    event: &Event,
    from_relay: bool,
    seen_on: Option<RelayUrl>,
    subscription: Option<String>,
) -> Result<(), Error> {
    let old = GLOBALS.events.get(&event.id).is_some();

    // Insert the event into globals map
    // (even if it was already seen)
    GLOBALS.events.insert(event.clone(), seen_on.clone());

    if old {
        tracing::trace!(
            "{}: Old Event: {} {:?} @{}",
            seen_on.as_ref().map(|r| r.as_str()).unwrap_or("_"),
            subscription.unwrap_or("_".to_string()),
            event.kind,
            event.created_at
        );

        // We already had this event. But we should still save it's "seen on " data
        if from_relay {
            if let Some(ref url) = seen_on {
                let now = Unixtime::now()?.0 as u64;

                // Save event_relay data
                let db_event_relay = DbEventRelay {
                    event: event.id.as_hex_string(),
                    relay: url.0.to_owned(),
                    when_seen: now,
                };
                DbEventRelay::replace(db_event_relay).await?;
            }
        }

        return Ok(());
    } else {
        tracing::debug!(
            "{}: New Event: {} {:?} @{}",
            seen_on.as_ref().map(|r| r.as_str()).unwrap_or("_"),
            subscription.unwrap_or("_".to_string()),
            event.kind,
            event.created_at
        );
    }

    // Save the event into the database
    if from_relay {
        // Convert a nostr Event into a DbEvent
        let db_event = DbEvent {
            id: event.id.into(),
            raw: serde_json::to_string(&event)?,
            pubkey: event.pubkey.into(),
            created_at: event.created_at.0,
            kind: event.kind.into(),
            content: event.content.clone(),
            ots: event.ots.clone(),
        };

        // Save into event table
        if event.kind.is_replaceable() {
            if !DbEvent::replace(db_event).await? {
                return Ok(()); // This did not replace anything.
            }
        } else if event.kind.is_parameterized_replaceable() {
            match event.parameter() {
                Some(param) => if ! DbEvent::replace_parameterized(db_event, param).await? {
                    return Ok(()); // This did not replace anything.
                },
                None => return Err("Parameterized event must have a parameter. This is a code issue, not a data issue".into()),
            };
        } else {
            DbEvent::insert(db_event).await?;
        }
    }

    if from_relay {
        if let Some(ref url) = seen_on {
            let now = Unixtime::now()?.0 as u64;

            // Save event_relay data
            let db_event_relay = DbEventRelay {
                event: event.id.as_hex_string(),
                relay: url.0.to_owned(),
                when_seen: now,
            };
            DbEventRelay::replace(db_event_relay).await?;

            // Create the person if missing in the database
            GLOBALS
                .people
                .create_all_if_missing(&[event.pubkey.into()])
                .await?;

            // Update person_relay.last_fetched
            DbPersonRelay::upsert_last_fetched(event.pubkey.as_hex_string(), url.to_owned(), now)
                .await?;
        }
    }

    // Save the tags into event_tag table
    if from_relay {
        for (seq, tag) in event.tags.iter().enumerate() {
            // Save into database
            {
                // convert to vec of strings
                let v: Vec<String> = serde_json::from_str(&serde_json::to_string(&tag)?)?;

                let db_event_tag = DbEventTag {
                    event: event.id.as_hex_string(),
                    seq: seq as u64,
                    label: v.get(0).cloned(),
                    field0: v.get(1).cloned(),
                    field1: v.get(2).cloned(),
                    field2: v.get(3).cloned(),
                    field3: v.get(4).cloned(),
                };
                DbEventTag::insert(db_event_tag).await?;
            }

            match tag {
                Tag::Event {
                    id: _,
                    recommended_relay_url: Some(should_be_url),
                    marker: _,
                } => {
                    if let Ok(url) = RelayUrl::try_from_unchecked_url(should_be_url) {
                        // Insert (or ignore) into relays table
                        let dbrelay = DbRelay::new(url);
                        DbRelay::insert(dbrelay).await?;
                    }
                }
                Tag::Pubkey {
                    pubkey,
                    recommended_relay_url: Some(should_be_url),
                    petname: _,
                } => {
                    if let Ok(url) = RelayUrl::try_from_unchecked_url(should_be_url) {
                        // Insert (or ignore) into relays table
                        let dbrelay = DbRelay::new(url.clone());
                        DbRelay::insert(dbrelay).await?;

                        // upsert person_relay.last_suggested_bytag
                        let now = Unixtime::now()?.0 as u64;
                        DbPersonRelay::upsert_last_suggested_bytag(
                            pubkey.to_string(),
                            url.clone(),
                            now,
                        )
                        .await?;
                    }
                }
                _ => {}
            }
        }
    }

    // Save event relationships
    {
        // replies to
        if let Some((id, _)) = event.replies_to() {
            if from_relay {
                let db_event_relationship = DbEventRelationship {
                    original: event.id.as_hex_string(),
                    refers_to: id.as_hex_string(),
                    relationship: "reply".to_string(),
                    content: None,
                };
                db_event_relationship.insert().await?;
            }

            // Insert into relationships
            Globals::add_relationship(id, event.id, Relationship::Reply).await;
        }

        // replies to root
        if let Some((id, _)) = event.replies_to_root() {
            if from_relay {
                let db_event_relationship = DbEventRelationship {
                    original: event.id.as_hex_string(),
                    refers_to: id.as_hex_string(),
                    relationship: "root".to_string(),
                    content: None,
                };
                db_event_relationship.insert().await?;
            }

            // Insert into relationships
            Globals::add_relationship(id, event.id, Relationship::Root).await;
        }

        // mentions
        for (id, _) in event.mentions() {
            if from_relay {
                let db_event_relationship = DbEventRelationship {
                    original: event.id.as_hex_string(),
                    refers_to: id.as_hex_string(),
                    relationship: "mention".to_string(),
                    content: None,
                };
                db_event_relationship.insert().await?;
            }

            // Insert into relationships
            Globals::add_relationship(id, event.id, Relationship::Mention).await;
        }

        // reacts to
        if let Some((id, reaction, _maybe_url)) = event.reacts_to() {
            if from_relay {
                let db_event_relationship = DbEventRelationship {
                    original: event.id.as_hex_string(),
                    refers_to: id.as_hex_string(),
                    relationship: "reaction".to_string(),
                    content: Some(reaction.clone()),
                };
                db_event_relationship.insert().await?;
            }

            // Insert into relationships
            Globals::add_relationship(id, event.id, Relationship::Reaction(reaction)).await;
        }

        // deletes
        if let Some((ids, reason)) = event.deletes() {
            for id in ids {
                if from_relay {
                    let db_event_relationship = DbEventRelationship {
                        original: event.id.as_hex_string(),
                        refers_to: id.as_hex_string(),
                        relationship: "deletion".to_string(),
                        content: Some(reason.clone()),
                        // FIXME: this table should have one more column for optional data
                    };
                    db_event_relationship.insert().await?;
                }

                // since it is a delete, we don't actually desire the event.

                // Insert into relationships
                Globals::add_relationship(id, event.id, Relationship::Deletion(reason.clone()))
                    .await;
            }
        }
    }

    // Save event_hashtags
    if from_relay {
        let hashtags = event.hashtags();
        for hashtag in hashtags {
            let db_event_hashtag = DbEventHashtag {
                event: event.id.as_hex_string(),
                hashtag: hashtag.clone(),
            };
            db_event_hashtag.insert().await?;
        }
    }

    // If metadata, update person
    if event.kind == EventKind::Metadata {
        let metadata: Metadata = serde_json::from_str(&event.content)?;

        GLOBALS
            .people
            .update_metadata(&event.pubkey.into(), metadata, event.created_at)
            .await?;
    }

    if event.kind == EventKind::ContactList {
        if let Some(pubkey) = GLOBALS.signer.public_key() {
            if event.pubkey == pubkey {
                process_your_contact_list(event).await?;
            } else {
                process_somebody_elses_contact_list(event).await?;
            }
        } else {
            process_somebody_elses_contact_list(event).await?;
        }
    }

    if event.kind == EventKind::RelayList {
        process_relay_list(event).await?;
    }

    // TBD (have to parse runes language for this)
    //if event.kind == EventKind::RelayList {
    //    process_somebody_elses_relay_list(event.pubkey.clone(), &event.contents).await?;
    //}

    // FIXME: Handle EventKind::RecommendedRelay

    Ok(())
}

async fn process_relay_list(event: &Event) -> Result<(), Error> {
    // Update that we received the relay list (and optionally bump forward the date
    // if this relay list happens to be newer)
    let newer = GLOBALS
        .people
        .update_relay_list_stamps(event.pubkey.into(), event.created_at.0)
        .await?;

    if !newer {
        return Ok(());
    }

    // Enable special handling for our own relay list
    let mut ours = false;
    if let Some(pubkey) = GLOBALS.signer.public_key() {
        if event.pubkey == pubkey {
            ours = true;

            tracing::info!("Processing our own relay list");

            // clear all read/write flags in relays (will be added back below)

            // in database
            DbRelay::clear_read_and_write().await?;

            // in memory
            for mut elem in GLOBALS.all_relays.iter_mut() {
                elem.value_mut().read = false;
                elem.value_mut().write = false;
            }
        }
    }

    let mut read_relays: Vec<RelayUrl> = Vec::new();
    let mut write_relays: Vec<RelayUrl> = Vec::new();
    for tag in event.tags.iter() {
        if let Tag::Reference { url, marker } = tag {
            if let Ok(relay_url) = RelayUrl::try_from_unchecked_url(url) {
                if let Some(m) = marker {
                    match &*m.trim().to_lowercase() {
                        "read" => {
                            read_relays.push(relay_url.clone());
                            if ours {
                                // set in database
                                DbRelay::update_read_and_write(relay_url.clone(), true, false)
                                    .await?;
                                // set in memory
                                if let Some(mut elem) = GLOBALS.all_relays.get_mut(&relay_url) {
                                    elem.read = true;
                                    elem.write = false;
                                }
                            }
                        }
                        "write" => {
                            write_relays.push(relay_url.clone());
                            if ours {
                                // set in database
                                DbRelay::update_read_and_write(relay_url.clone(), false, true)
                                    .await?;
                                // set in memory
                                if let Some(mut elem) = GLOBALS.all_relays.get_mut(&relay_url) {
                                    elem.read = false;
                                    elem.write = true;
                                }
                            }
                        }
                        _ => {} // ignore unknown marker
                    }
                } else {
                    read_relays.push(relay_url.clone());
                    write_relays.push(relay_url.clone());
                    if ours {
                        // set in database
                        DbRelay::update_read_and_write(relay_url.clone(), true, true).await?;
                        // set in memory
                        if let Some(mut elem) = GLOBALS.all_relays.get_mut(&relay_url) {
                            elem.read = true;
                            elem.write = true;
                        }
                    }
                }
            }
        }
    }

    DbPersonRelay::set_relay_list(event.pubkey.into(), read_relays, write_relays).await?;

    Ok(())
}

async fn process_somebody_elses_contact_list(event: &Event) -> Result<(), Error> {
    // We don't keep their contacts or show to the user yet.
    // We only process the contents for (non-standard) relay list information.

    // Try to parse the contents as a SimpleRelayList (ignore if it is not)
    if let Ok(srl) = serde_json::from_str::<SimpleRelayList>(&event.content) {
        // Update that we received the relay list (and optionally bump forward the date
        // if this relay list happens to be newer)
        let newer = GLOBALS
            .people
            .update_relay_list_stamps(event.pubkey.into(), event.created_at.0)
            .await?;

        if !newer {
            return Ok(());
        }

        let mut read_relays: Vec<RelayUrl> = Vec::new();
        let mut write_relays: Vec<RelayUrl> = Vec::new();
        for (url, simple_relay_usage) in srl.0.iter() {
            if let Ok(relay_url) = RelayUrl::try_from_unchecked_url(url) {
                if simple_relay_usage.read {
                    read_relays.push(relay_url.clone());
                }
                if simple_relay_usage.write {
                    write_relays.push(relay_url.clone());
                }
            }
        }
        DbPersonRelay::set_relay_list(event.pubkey.into(), read_relays, write_relays).await?;
    }

    Ok(())
}

async fn process_your_contact_list(event: &Event) -> Result<(), Error> {
    // Only process if it is newer than what we already have
    if event.created_at.0
        > GLOBALS
            .people
            .last_contact_list_asof
            .load(Ordering::Relaxed)
    {
        GLOBALS
            .people
            .last_contact_list_asof
            .store(event.created_at.0, Ordering::Relaxed);

        let merge: bool = GLOBALS.pull_following_merge.load(Ordering::Relaxed);
        let mut pubkeys: Vec<PublicKeyHex> = Vec::new();

        let now = Unixtime::now().unwrap();

        // 'p' tags represent the author's contacts
        for tag in &event.tags {
            if let Tag::Pubkey {
                pubkey,
                recommended_relay_url,
                petname: _,
            } = tag
            {
                // Save the pubkey for actual following them (outside of the loop in a batch)
                pubkeys.push(pubkey.to_owned());

                // If there is a URL, create or update person_relay last_suggested_kind3
                if let Some(url) = recommended_relay_url
                    .as_ref()
                    .and_then(|rru| RelayUrl::try_from_unchecked_url(rru).ok())
                {
                    DbPersonRelay::upsert_last_suggested_kind3(
                        pubkey.to_string(),
                        url,
                        now.0 as u64,
                    )
                    .await?;
                }

                // TBD: do something with the petname
            }
        }

        // Follow all those pubkeys, and unfollow everbody else if merge=false
        // (and the date is used to ignore if the data is outdated)
        GLOBALS
            .people
            .follow_all(&pubkeys, merge, event.created_at)
            .await?;

        // Trigger the overlord to pick relays again
        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PickRelays);
    }

    Ok(())
}
