use crate::comms::ToOverlordMessage;
use crate::error::Error;
use crate::filter::EventFilterAction;
use crate::globals::GLOBALS;
use crate::person_relay::PersonRelay;
use async_recursion::async_recursion;
use nostr_types::{
    Event, EventKind, Metadata, NostrBech32, PublicKey, RelayUrl, SimpleRelayList, Tag, Unixtime,
};
use std::sync::atomic::Ordering;

// This processes a new event, saving the results into the database
// and also populating the GLOBALS maps.
#[async_recursion]
pub async fn process_new_event(
    event: &Event,
    seen_on: Option<RelayUrl>,
    subscription: Option<String>,
    verify: bool,
) -> Result<(), Error> {
    let now = Unixtime::now()?;

    // Bump count
    GLOBALS.events_processed.fetch_add(1, Ordering::SeqCst);

    // Detect if duplicate. We still need to process some things even if a duplicate
    let duplicate = GLOBALS.storage.has_event(event.id)?;

    // Verify the event,
    // Don't verify if it is a duplicate:
    //    NOTE: relays could send forged events with valid IDs of other events, but if
    //          they do that in an event that is a duplicate of one we already have, this
    //          duplicate will only affect seen-on information, it will not be saved.
    if !duplicate && verify {
        let mut maxtime = now;
        maxtime.0 += GLOBALS.storage.read_setting_future_allowance_secs() as i64;
        if let Err(e) = event.verify(Some(maxtime)) {
            tracing::error!("{}: VERIFY ERROR: {}", e, serde_json::to_string(&event)?);
            return Ok(());
        }
    }

    if let Some(url) = &seen_on {
        // Save seen-on-relay information
        GLOBALS
            .storage
            .add_event_seen_on_relay(event.id, url, now, None)?;

        // Create the person if missing in the database
        GLOBALS
            .storage
            .write_person_if_missing(&event.pubkey, None)?;

        // Update person-relay information (seen them on this relay)
        let mut pr = match GLOBALS.storage.read_person_relay(event.pubkey, url)? {
            Some(pr) => pr,
            None => PersonRelay::new(event.pubkey, url.clone()),
        };
        pr.last_fetched = Some(now.0 as u64);
        GLOBALS.storage.write_person_relay(&pr, None)?;
    }

    // Spam filter (displayable and author is not followed)
    if event.kind.is_feed_displayable() && !GLOBALS.people.is_followed(&event.pubkey) {
        let author = GLOBALS.storage.read_person(&event.pubkey)?;
        match crate::filter::filter(event.clone(), author) {
            EventFilterAction::Allow => {}
            EventFilterAction::Deny => {
                tracing::info!(
                    "SPAM FILTER: Filtered out event {}",
                    event.id.as_hex_string()
                );
                return Ok(());
            }
            EventFilterAction::MuteAuthor => {
                GLOBALS.people.mute(&event.pubkey, true)?;
                return Ok(());
            }
        }
    }

    // Determine if we already had this event
    if duplicate {
        tracing::trace!(
            "{}: Old Event: {} {:?} @{}",
            seen_on.as_ref().map(|r| r.as_str()).unwrap_or("_"),
            subscription.as_ref().unwrap_or(&"_".to_string()),
            event.kind,
            event.created_at
        );
        return Ok(()); // No more processing needed for existing event.
    }

    // Save event
    // Bail if the event is an already-replaced replaceable event
    if event.kind.is_replaceable() {
        if !GLOBALS.storage.replace_event(event, None)? {
            tracing::trace!(
                "{}: Old Event: {} {:?} @{}",
                seen_on.as_ref().map(|r| r.as_str()).unwrap_or("_"),
                subscription.as_ref().unwrap_or(&"_".to_string()),
                event.kind,
                event.created_at
            );
            return Ok(()); // This did not replace anything.
        }
    } else if event.kind.is_parameterized_replaceable() {
        if !GLOBALS.storage.replace_parameterized_event(event, None)? {
            tracing::trace!(
                "{}: Old Event: {} {:?} @{}",
                seen_on.as_ref().map(|r| r.as_str()).unwrap_or("_"),
                subscription.as_ref().unwrap_or(&"_".to_string()),
                event.kind,
                event.created_at
            );
            return Ok(()); // This did not replace anything.
        }
    } else {
        // This will ignore if it is already there
        GLOBALS.storage.write_event(event, None)?;
    }

    // Log
    tracing::debug!(
        "{}: New Event: {} {:?} @{}",
        seen_on.as_ref().map(|r| r.as_str()).unwrap_or("_"),
        subscription.as_ref().unwrap_or(&"_".to_string()),
        event.kind,
        event.created_at
    );

    // If it is a GiftWrap, from here on out operate on the Rumor
    let mut event: &Event = event; // take ownership of this reference
    let mut rumor_event: Event;
    if event.kind == EventKind::GiftWrap {
        let rumor = GLOBALS.signer.unwrap_giftwrap(event)?;
        rumor_event = rumor.into_event_with_bad_signature();
        rumor_event.id = event.id; // Lie so it's handled with the giftwrap's id
        event = &rumor_event;
    }

    if seen_on.is_some() {
        for tag in event.tags.iter() {
            match tag {
                Tag::Event {
                    recommended_relay_url: Some(should_be_url),
                    ..
                } => {
                    if let Ok(url) = RelayUrl::try_from_unchecked_url(should_be_url) {
                        GLOBALS.storage.write_relay_if_missing(&url, None)?;
                    }
                }
                Tag::Pubkey {
                    pubkey,
                    recommended_relay_url: Some(should_be_url),
                    ..
                } => {
                    if let Ok(pubkey) = PublicKey::try_from_hex_string(pubkey, true) {
                        if let Ok(url) = RelayUrl::try_from_unchecked_url(should_be_url) {
                            GLOBALS.storage.write_relay_if_missing(&url, None)?;

                            // Add person if missing
                            GLOBALS.people.create_all_if_missing(&[pubkey])?;

                            // upsert person_relay.last_suggested_bytag
                            let mut pr = match GLOBALS.storage.read_person_relay(pubkey, &url)? {
                                Some(pr) => pr,
                                None => PersonRelay::new(pubkey, url.clone()),
                            };
                            pr.last_suggested_bytag = Some(now.0 as u64);
                            GLOBALS.storage.write_person_relay(&pr, None)?;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Save event_hashtags
    if seen_on.is_some() {
        let hashtags = event.hashtags();
        for hashtag in hashtags {
            GLOBALS.storage.add_hashtag(&hashtag, event.id, None)?;
        }
    }

    // Save event relationships (whether from a relay or not)
    let invalid_ids = GLOBALS
        .storage
        .process_relationships_of_event(event, None)?;

    // Invalidate UI events indicated by those relationships
    GLOBALS.ui_notes_to_invalidate.write().extend(&invalid_ids);

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
        GLOBALS.storage.process_relay_list(event)?;
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
            .set_relay_list(event.pubkey, inbox_relays, outbox_relays, None)?;
    }

    Ok(())
}
