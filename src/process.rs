use crate::comms::ToOverlordMessage;
use crate::db::{DbEvent, DbEventRelay, DbEventTag, DbPersonRelay, DbRelay};
use crate::error::Error;
use crate::globals::{Globals, GLOBALS};
use crate::relationship::Relationship;
use nostr_types::{
    Event, EventKind, Metadata, NostrBech32, RelayUrl, SimpleRelayList, Tag, Unixtime,
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
    let now = Unixtime::now()?;

    // If it was from a relay,
    // Insert into database; bail if event is an already-replaced replaceable event.
    if from_relay {
        // Convert a nostr Event into a DbEvent
        let db_event = DbEvent {
            id: event.id.into(),
            raw: serde_json::to_string(&event)?,
            pubkey: event.pubkey.into(),
            created_at: event.created_at.0,
            kind: {
                let k: u32 = event.kind.into();
                k.into()
            },
            content: event.content.clone(),
            ots: event.ots.clone(),
        };

        if event.kind.is_replaceable() {
            if !DbEvent::replace(db_event).await? {
                tracing::trace!(
                    "{}: Old Event: {} {:?} @{}",
                    seen_on.as_ref().map(|r| r.as_str()).unwrap_or("_"),
                    subscription.unwrap_or("_".to_string()),
                    event.kind,
                    event.created_at
                );
                return Ok(()); // This did not replace anything.
            }
        } else if event.kind.is_parameterized_replaceable() {
            match event.parameter() {
                Some(param) => if ! DbEvent::replace_parameterized(db_event, param).await? {
                    tracing::trace!(
                        "{}: Old Event: {} {:?} @{}",
                        seen_on.as_ref().map(|r| r.as_str()).unwrap_or("_"),
                        subscription.unwrap_or("_".to_string()),
                        event.kind,
                        event.created_at
                    );
                    return Ok(()); // This did not replace anything.
                },
                None => return Err("Parameterized event must have a parameter. This is a code issue, not a data issue".into()),
            }
        } else {
            // This will ignore if it is already there
            DbEvent::insert(db_event).await?;
        }
    }

    let old = GLOBALS.events.get(&event.id).is_some();
    // If we don't already have it
    if !old {
        // Insert into map (memory only)
        // This also inserts the 'seen_on' relay information
        GLOBALS.events.insert(event.clone(), seen_on.clone());
    } else {
        // Just insert the new seen_on information (memory only)
        if let Some(url) = &seen_on {
            GLOBALS.events.add_seen_on(event.id, url);
        }
    }

    if let Some(ref url) = seen_on {
        // Insert into event_relay "seen" relationship (database)
        if from_relay {
            let db_event_relay = DbEventRelay {
                id: event.id,
                relay: url.to_owned(),
                when_seen: now,
            };
            if let Err(e) = db_event_relay.save() {
                tracing::error!(
                    "Error saving relay of old-event {} {}: {}",
                    event.id.as_hex_string(),
                    url.0,
                    e
                );
            }

            // Create the person if missing in the database
            GLOBALS
                .people
                .create_all_if_missing(&[event.pubkey.into()])
                .await?;

            // Update person_relay.last_fetched
            DbPersonRelay::upsert_last_fetched(
                event.pubkey.as_hex_string(),
                url.to_owned(),
                now.0 as u64,
            )
            .await?;
        }
    }

    // Log
    if old {
        tracing::trace!(
            "{}: Old Event: {} {:?} @{}",
            seen_on.as_ref().map(|r| r.as_str()).unwrap_or("_"),
            subscription.unwrap_or("_".to_string()),
            event.kind,
            event.created_at
        );
        return Ok(()); // No more processing needed for existing event.
    } else {
        tracing::debug!(
            "{}: New Event: {} {:?} @{}",
            seen_on.as_ref().map(|r| r.as_str()).unwrap_or("_"),
            subscription.unwrap_or("_".to_string()),
            event.kind,
            event.created_at
        );
    }

    if from_relay {
        // Save the tags into event_tag table
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
                    recommended_relay_url: Some(should_be_url),
                    ..
                } => {
                    if let Ok(url) = RelayUrl::try_from_unchecked_url(should_be_url) {
                        GLOBALS.storage.write_relay_if_missing(&url)?;
                    }
                }
                Tag::Pubkey {
                    pubkey,
                    recommended_relay_url: Some(should_be_url),
                    ..
                } => {
                    if let Ok(url) = RelayUrl::try_from_unchecked_url(should_be_url) {
                        GLOBALS.storage.write_relay_if_missing(&url)?;

                        // Add person if missing
                        GLOBALS
                            .people
                            .create_all_if_missing(&[pubkey.clone()])
                            .await?;

                        // upsert person_relay.last_suggested_bytag
                        DbPersonRelay::upsert_last_suggested_bytag(
                            pubkey.to_string(),
                            url,
                            now.0 as u64,
                        )
                        .await?;
                    }
                }
                _ => {}
            }
        }
    }

    // Save event relationships (whether from relay or not)
    {
        // replies to
        if let Some((id, _)) = event.replies_to() {
            // Insert into relationships
            Globals::add_relationship(id, event.id, Relationship::Reply).await;
        }

        /*
        // replies to root
        if let Some((id, _)) = event.replies_to_root() {
            // Insert into relationships
            Globals::add_relationship(id, event.id, Relationship::Root).await;
        }

        // mentions
        for (id, _) in event.mentions() {
            // Insert into relationships
            Globals::add_relationship(id, event.id, Relationship::Mention).await;
        }
         */

        // reacts to
        if let Some((id, reaction, _maybe_url)) = event.reacts_to() {
            // Insert into relationships
            Globals::add_relationship(id, event.id, Relationship::Reaction(reaction)).await;

            // UI cache invalidation (so the note get rerendered)
            GLOBALS.ui_notes_to_invalidate.write().push(id);
        }

        // deletes
        if let Some((ids, reason)) = event.deletes() {
            // UI cache invalidation (so the notes get rerendered)
            GLOBALS.ui_notes_to_invalidate.write().extend(&ids);

            for id in ids {
                // since it is a delete, we don't actually desire the event.

                // Insert into relationships
                Globals::add_relationship(id, event.id, Relationship::Deletion(reason.clone()))
                    .await;
            }
        }

        // zaps
        match event.zaps() {
            Ok(Some(zapdata)) => {
                // Insert into relationships
                Globals::add_relationship(
                    zapdata.id,
                    event.id,
                    Relationship::ZapReceipt(zapdata.amount),
                )
                .await;

                // UI cache invalidation (so the note gets rerendered)
                GLOBALS.ui_notes_to_invalidate.write().push(zapdata.id);
            }
            Err(e) => tracing::error!("Invalid zap receipt: {}", e),
            _ => {}
        }
    }

    // Save event_hashtags
    if from_relay {
        let hashtags = event.hashtags();
        for hashtag in hashtags {
            GLOBALS.storage.add_hashtag(&hashtag, event.id)?;
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
                // We do not process our own contact list automatically.
                // Instead we only process it on user command.
                // See Overlord::update_following()
                //
                // But we do update people.last_contact_list_asof and _size
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
                    let size = event
                        .tags
                        .iter()
                        .filter(|t| matches!(t, Tag::Pubkey { .. }))
                        .count();
                    GLOBALS
                        .people
                        .last_contact_list_size
                        .store(size, Ordering::Relaxed);
                }
                return Ok(());
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

    // If the content contains an nevent and we don't have it, fetch it from those relays
    for bech32 in NostrBech32::find_all_in_string(&event.content) {
        if let NostrBech32::EventPointer(ep) = bech32 {
            if GLOBALS.events.get(&ep.id).is_none() {
                let relay_urls: Vec<RelayUrl> = ep
                    .relays
                    .iter()
                    .filter_map(|unchecked| RelayUrl::try_from_unchecked_url(unchecked).ok())
                    .collect();
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::FetchEvent(ep.id, relay_urls));
            }
        }
        // TBD: If the content contains an nprofile, make sure the pubkey is associated
        // with those relays
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

            // clear all read/write flags from relays (will be added back below)
            DbRelay::clear_all_relay_list_usage_bits()?;
        }
    }

    let mut inbox_relays: Vec<RelayUrl> = Vec::new();
    let mut outbox_relays: Vec<RelayUrl> = Vec::new();
    for tag in event.tags.iter() {
        if let Tag::Reference { url, marker, .. } = tag {
            if let Ok(relay_url) = RelayUrl::try_from_unchecked_url(url) {
                if let Some(m) = marker {
                    match &*m.trim().to_lowercase() {
                        "read" => {
                            // 'read' means inbox and not outbox
                            inbox_relays.push(relay_url.clone());
                            if ours {
                                if let Some(mut dbrelay) = GLOBALS.storage.read_relay(&relay_url)? {
                                    // Update
                                    dbrelay.set_usage_bits(DbRelay::INBOX);
                                    dbrelay.clear_usage_bits(DbRelay::OUTBOX);
                                    GLOBALS.storage.write_relay(&dbrelay)?;
                                } else {
                                    // Insert missing relay
                                    let mut dbrelay = DbRelay::new(relay_url.to_owned());
                                    // Since we are creating, we add READ
                                    dbrelay.set_usage_bits(DbRelay::INBOX | DbRelay::READ);
                                    GLOBALS.storage.write_relay(&dbrelay)?;
                                }
                            }
                        }
                        "write" => {
                            // 'write' means outbox and not inbox
                            outbox_relays.push(relay_url.clone());
                            if ours {
                                if let Some(mut dbrelay) = GLOBALS.storage.read_relay(&relay_url)? {
                                    // Update
                                    dbrelay.set_usage_bits(DbRelay::OUTBOX);
                                    dbrelay.clear_usage_bits(DbRelay::INBOX);
                                    GLOBALS.storage.write_relay(&dbrelay)?;
                                } else {
                                    // Create
                                    let mut dbrelay = DbRelay::new(relay_url.to_owned());
                                    // Since we are creating, we add WRITE
                                    dbrelay.set_usage_bits(DbRelay::OUTBOX | DbRelay::WRITE);
                                    GLOBALS.storage.write_relay(&dbrelay)?;
                                }
                            }
                        }
                        _ => {} // ignore unknown marker
                    }
                } else {
                    // No marker means both inbox and outbox
                    inbox_relays.push(relay_url.clone());
                    outbox_relays.push(relay_url.clone());
                    if ours {
                        if let Some(mut dbrelay) = GLOBALS.storage.read_relay(&relay_url)? {
                            // Update
                            dbrelay.set_usage_bits(DbRelay::INBOX | DbRelay::OUTBOX);
                            GLOBALS.storage.write_relay(&dbrelay)?;
                        } else {
                            // Create
                            let mut dbrelay = DbRelay::new(relay_url.to_owned());
                            // Since we are creating, we add READ and WRITE
                            dbrelay.set_usage_bits(
                                DbRelay::INBOX | DbRelay::OUTBOX | DbRelay::READ | DbRelay::WRITE,
                            );
                            GLOBALS.storage.write_relay(&dbrelay)?;
                        }
                    }
                }
            }
        }
    }

    DbPersonRelay::set_relay_list(event.pubkey.into(), inbox_relays, outbox_relays).await?;

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

        let mut inbox_relays: Vec<RelayUrl> = Vec::new();
        let mut outbox_relays: Vec<RelayUrl> = Vec::new();
        for (url, simple_relay_usage) in srl.0.iter() {
            if let Ok(relay_url) = RelayUrl::try_from_unchecked_url(url) {
                if simple_relay_usage.read {
                    inbox_relays.push(relay_url.clone());
                }
                if simple_relay_usage.write {
                    outbox_relays.push(relay_url.clone());
                }
            }
        }
        DbPersonRelay::set_relay_list(event.pubkey.into(), inbox_relays, outbox_relays).await?;
    }

    Ok(())
}
