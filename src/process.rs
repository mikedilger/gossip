use crate::comms::ToOverlordMessage;
use crate::error::Error;
use crate::globals::GLOBALS;
use crate::person_relay::PersonRelay;
use crate::relay::Relay;
use nostr_types::{
    Event, EventKind, Metadata, NostrBech32, PublicKey, RelayUrl, SimpleRelayList, Tag, Unixtime,
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

    // Save seen-on-relay information
    if let Some(url) = &seen_on {
        if from_relay {
            GLOBALS
                .storage
                .add_event_seen_on_relay(event.id, url, now)?;
        }
    }

    // Determine if we already had this event
    let duplicate = GLOBALS.storage.read_event(event.id)?.is_some();
    if duplicate {
        tracing::trace!(
            "{}: Old Event: {} {:?} @{}",
            seen_on.as_ref().map(|r| r.as_str()).unwrap_or("_"),
            subscription.unwrap_or("_".to_string()),
            event.kind,
            event.created_at
        );
        return Ok(()); // No more processing needed for existing event.
    }

    // Save event
    // Bail if the event is an already-replaced replaceable event
    if from_relay {
        if event.kind.is_replaceable() {
            if !GLOBALS.storage.replace_event(event)? {
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
            if !GLOBALS.storage.replace_parameterized_event(event)? {
                tracing::trace!(
                    "{}: Old Event: {} {:?} @{}",
                    seen_on.as_ref().map(|r| r.as_str()).unwrap_or("_"),
                    subscription.unwrap_or("_".to_string()),
                    event.kind,
                    event.created_at
                );
                return Ok(()); // This did not replace anything.
            }
        } else {
            // This will ignore if it is already there
            GLOBALS.storage.write_event(event)?;
        }
    }

    // Log
    tracing::debug!(
        "{}: New Event: {} {:?} @{}",
        seen_on.as_ref().map(|r| r.as_str()).unwrap_or("_"),
        subscription.unwrap_or("_".to_string()),
        event.kind,
        event.created_at
    );

    if from_relay {
        // Create the person if missing in the database
        GLOBALS.people.create_all_if_missing(&[event.pubkey])?;

        if let Some(ref url) = seen_on {
            // Update person_relay.last_fetched
            let mut pr = match GLOBALS.storage.read_person_relay(event.pubkey, url)? {
                Some(pr) => pr,
                None => PersonRelay::new(event.pubkey, url.clone()),
            };
            pr.last_fetched = Some(now.0 as u64);
            GLOBALS.storage.write_person_relay(&pr)?;
        }

        // Save the tags into event_tag table
        GLOBALS.storage.write_event_tags(event)?;

        for tag in event.tags.iter() {
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
                    if let Ok(pubkey) = PublicKey::try_from_hex_string(pubkey) {
                        if let Ok(url) = RelayUrl::try_from_unchecked_url(should_be_url) {
                            GLOBALS.storage.write_relay_if_missing(&url)?;

                            // Add person if missing
                            GLOBALS.people.create_all_if_missing(&[pubkey])?;

                            // upsert person_relay.last_suggested_bytag
                            let mut pr = match GLOBALS.storage.read_person_relay(pubkey, &url)? {
                                Some(pr) => pr,
                                None => PersonRelay::new(pubkey, url.clone()),
                            };
                            pr.last_suggested_bytag = Some(now.0 as u64);
                            GLOBALS.storage.write_person_relay(&pr)?;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Save event relationships (whether from a relay or not)
    let invalid_ids = GLOBALS.storage.process_relationships_of_event(event)?;

    // Invalidate UI events indicated by those relationships
    GLOBALS.ui_notes_to_invalidate.write().extend(&invalid_ids);

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
            .update_metadata(&event.pubkey, metadata, event.created_at)
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
            if GLOBALS.storage.read_event(ep.id)?.is_none() {
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
        .update_relay_list_stamps(event.pubkey, event.created_at.0)
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
            Relay::clear_all_relay_list_usage_bits()?;
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
                                    dbrelay.set_usage_bits(Relay::INBOX);
                                    dbrelay.clear_usage_bits(Relay::OUTBOX);
                                    GLOBALS.storage.write_relay(&dbrelay)?;
                                } else {
                                    // Insert missing relay
                                    let mut dbrelay = Relay::new(relay_url.to_owned());
                                    // Since we are creating, we add READ
                                    dbrelay.set_usage_bits(Relay::INBOX | Relay::READ);
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
                                    dbrelay.set_usage_bits(Relay::OUTBOX);
                                    dbrelay.clear_usage_bits(Relay::INBOX);
                                    GLOBALS.storage.write_relay(&dbrelay)?;
                                } else {
                                    // Create
                                    let mut dbrelay = Relay::new(relay_url.to_owned());
                                    // Since we are creating, we add WRITE
                                    dbrelay.set_usage_bits(Relay::OUTBOX | Relay::WRITE);
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
                            dbrelay.set_usage_bits(Relay::INBOX | Relay::OUTBOX);
                            GLOBALS.storage.write_relay(&dbrelay)?;
                        } else {
                            // Create
                            let mut dbrelay = Relay::new(relay_url.to_owned());
                            // Since we are creating, we add READ and WRITE
                            dbrelay.set_usage_bits(
                                Relay::INBOX | Relay::OUTBOX | Relay::READ | Relay::WRITE,
                            );
                            GLOBALS.storage.write_relay(&dbrelay)?;
                        }
                    }
                }
            }
        }
    }

    GLOBALS
        .storage
        .set_relay_list(event.pubkey, inbox_relays, outbox_relays)?;

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
            .update_relay_list_stamps(event.pubkey, event.created_at.0)
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
        GLOBALS
            .storage
            .set_relay_list(event.pubkey, inbox_relays, outbox_relays)?;
    }

    Ok(())
}
