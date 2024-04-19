use crate::comms::ToOverlordMessage;
use crate::error::Error;
use crate::filter::EventFilterAction;
use crate::globals::GLOBALS;
use crate::misc::Freshness;
use crate::people::{People, PersonList, PersonListMetadata};
use crate::person_relay::PersonRelay;
use crate::relationship::{RelationshipByAddr, RelationshipById};
use async_recursion::async_recursion;
use heed::RwTxn;
use nostr_types::{
    Event, EventAddr, EventKind, EventReference, Id, Metadata, NostrBech32, PublicKey, RelayList,
    RelayUrl, RelayUsage, SimpleRelayList, Tag, Unixtime,
};
use std::sync::atomic::Ordering;

/// This is mainly used internally to gossip-lib, but you can use it to stuff events
/// into gossip from other sources. This processes a new event, saving the results into
/// the database and also populating the GLOBALS maps.
#[async_recursion]
pub async fn process_new_event(
    event: &Event,
    seen_on: Option<RelayUrl>,
    subscription: Option<String>,
    verify: bool,
    process_even_if_duplicate: bool,
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
    if (event.kind.is_feed_displayable() || event.kind == EventKind::GiftWrap)
        && !GLOBALS
            .people
            .is_person_in_list(&event.pubkey, PersonList::Followed)
    {
        let filter_result = {
            if event.kind == EventKind::GiftWrap {
                if let Ok(rumor) = GLOBALS.identity.unwrap_giftwrap(event) {
                    let author = GLOBALS.storage.read_person(&rumor.pubkey)?;
                    Some(crate::filter::filter_rumor(rumor, author, event.id))
                } else {
                    None
                }
            } else {
                let author = GLOBALS.storage.read_person(&event.pubkey)?;
                Some(crate::filter::filter_event(event.clone(), author))
            }
        };

        match filter_result {
            None => {}
            Some(EventFilterAction::Allow) => {}
            Some(EventFilterAction::Deny) => {
                tracing::info!(
                    "SPAM FILTER: Filtered out event {}",
                    event.id.as_hex_string()
                );
                return Ok(());
            }
            Some(EventFilterAction::MuteAuthor) => {
                let public = true;
                GLOBALS.people.mute(&event.pubkey, true, public)?;
                return Ok(());
            }
        }
    }

    // Invalidate the note itself (due to seen_on probably changing)
    GLOBALS.ui_notes_to_invalidate.write().push(event.id);

    // Determine if we already had this event
    if duplicate && !process_even_if_duplicate {
        tracing::trace!(
            "{}: Old Event: {} {:?} @{}",
            seen_on.as_ref().map(|r| r.as_str()).unwrap_or("_"),
            subscription.as_ref().unwrap_or(&"_".to_string()),
            event.kind,
            event.created_at
        );
        return Ok(()); // No more processing needed for existing event.
    }

    // Ignore if the event is already deleted (by id)
    for (_id, relbyid) in GLOBALS.storage.find_relationships_by_id(event.id)? {
        if let RelationshipById::Deletion { by, reason: _ } = relbyid {
            if event.delete_author_allowed(by) {
                tracing::trace!(
                    "{}: Deleted Event: {} {:?} @{}",
                    seen_on.as_ref().map(|r| r.as_str()).unwrap_or("_"),
                    subscription.as_ref().unwrap_or(&"_".to_string()),
                    event.kind,
                    event.created_at
                );
                return Ok(());
            }
        }
    }

    // Ignore if the event is already deleted (by address)
    if let Some(parameter) = event.parameter() {
        let ea = EventAddr {
            d: parameter.to_owned(),
            relays: vec![],
            kind: event.kind,
            author: event.pubkey,
        };
        for (_id, relbyaddr) in GLOBALS.storage.find_relationships_by_addr(&ea)? {
            if let RelationshipByAddr::Deletion { by, reason: _ } = relbyaddr {
                if by == event.pubkey {
                    tracing::trace!(
                        "{}: Deleted Event: {} {:?} @{}",
                        seen_on.as_ref().map(|r| r.as_str()).unwrap_or("_"),
                        subscription.as_ref().unwrap_or(&"_".to_string()),
                        event.kind,
                        event.created_at
                    );
                    return Ok(());
                }
            }
        }
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

    // If we were searching for this event, add it to the search results
    let is_a_search_result: bool = GLOBALS.events_being_searched_for.read().contains(&event.id);
    if is_a_search_result {
        GLOBALS
            .events_being_searched_for
            .write()
            .retain(|id| *id != event.id);
        GLOBALS.note_search_results.write().push(event.clone());
    }
    // FIXME do same for event addr

    // If it is a GiftWrap, from here on out operate on the Rumor with the giftwrap's id
    let mut event: &Event = event; // take ownership of this reference
    let mut rumor_event: Event;
    if event.kind == EventKind::GiftWrap {
        let rumor = GLOBALS.identity.unwrap_giftwrap(event)?;
        rumor_event = rumor.into_event_with_bad_signature();
        rumor_event.id = event.id; // Lie so it's handled with the giftwrap's id
        event = &rumor_event;
    }

    if seen_on.is_some() {
        for tag in event.tags.iter() {
            if let Ok((_, Some(uurl), _optmarker)) = tag.parse_event() {
                if let Ok(url) = RelayUrl::try_from_unchecked_url(&uurl) {
                    GLOBALS.storage.write_relay_if_missing(&url, None)?;
                }
            }

            if let Ok((pubkey, maybeurl, _)) = tag.parse_pubkey() {
                // Add person if missing
                GLOBALS.people.create_all_if_missing(&[pubkey])?;

                if let Some(uncheckedurl) = maybeurl {
                    if let Ok(url) = RelayUrl::try_from_unchecked_url(&uncheckedurl) {
                        GLOBALS.storage.write_relay_if_missing(&url, None)?;

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
        }
    }

    // Save event relationships (whether from a relay or not)
    let invalid_ids = process_relationships_of_event(event, None)?;

    // Invalidate UI events indicated by those relationships
    GLOBALS.ui_notes_to_invalidate.write().extend(&invalid_ids);

    // Let seeker know about this event id (in case it was sought)
    GLOBALS.seeker.found_or_cancel(event.id);

    // If metadata, update person
    if event.kind == EventKind::Metadata {
        let metadata: Metadata = serde_json::from_str(&event.content)?;

        GLOBALS
            .people
            .update_metadata(&event.pubkey, metadata, event.created_at)
            .await?;
    }

    if event.kind == EventKind::ContactList {
        process_contact_list(event).await?;0
        if let Some(pubkey) = GLOBALS.identity.public_key() {
            if event.pubkey == pubkey {
                // Updates stamps and counts, does NOT change membership
                let (_personlist, _metadata) =
                    update_or_allocate_person_list_from_event(event, pubkey)?;
            } else {
                process_somebody_elses_contact_list(event).await?;
            }
        } else {
            process_somebody_elses_contact_list(event).await?;
        }
    } else if event.kind == EventKind::MuteList || event.kind == EventKind::FollowSets {
        // Only our own
        if let Some(pubkey) = GLOBALS.identity.public_key() {
            if event.pubkey == pubkey {
                // Updates stamps and counts, does NOT change membership
                let (_personlist, _metadata) =
                    update_or_allocate_person_list_from_event(event, pubkey)?;
            }
        }
    } else if event.kind == EventKind::RelayList {
        GLOBALS.storage.process_relay_list(event)?;

        // Let the seeker know we now have relays for this author, in case the seeker
        // wants to update it's state
        // (we might not, but by this point we have tried)
        GLOBALS.seeker.found_author_relays(event.pubkey);

        // the following also refreshes scores before it picks relays
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::RefreshScoresAndPickRelays);
    } else if event.kind == EventKind::Repost {
        // If it has a json encoded inner event
        if let Ok(inner_event) = serde_json::from_str::<Event>(&event.content) {
            // Maybe seek the relay list of the event author
            match People::person_needs_relay_list(inner_event.pubkey) {
                Freshness::NeverSought | Freshness::Stale => {
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::SubscribeDiscover(
                            vec![inner_event.pubkey],
                            None,
                        ));
                }
                _ => {}
            }

            // process the inner event
            process_new_event(&inner_event, None, None, verify, false).await?;

            // Seek additional info for this event by id and author
            GLOBALS
                .seeker
                .seek_id_and_author(inner_event.id, inner_event.pubkey, vec![])?;
        } else {
            // If the content is a repost, seek the event it reposts
            for eref in event.mentions().iter() {
                match eref {
                    EventReference::Id { id, relays, .. } => {
                        if relays.is_empty() {
                            // Even if the event tags the author, we have no way to coorelate
                            // the nevent with that tag.
                            GLOBALS.seeker.seek_id(*id, vec![])?;
                        } else {
                            GLOBALS.seeker.seek_id_and_relays(*id, relays.clone());
                        }
                    }
                    EventReference::Addr(ea) => {
                        if !ea.relays.is_empty() {
                            let _ = GLOBALS
                                .to_overlord
                                .send(ToOverlordMessage::FetchEventAddr(ea.clone()));
                        }
                    }
                }
            }
        }
    } else if event.kind == EventKind::NostrConnect {
        crate::nip46::handle_command(event, seen_on.clone())?
    }

    if event.kind.is_feed_displayable() {
        // Process the content for references to things we might want
        for bech32 in NostrBech32::find_all_in_string(&event.content) {
            match bech32 {
                NostrBech32::Id(id) => {
                    if GLOBALS.storage.read_event(id)?.is_none() {
                        if let Some(relay_url) = seen_on.as_ref() {
                            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FetchEvent(
                                id,
                                vec![relay_url.to_owned()],
                            ));
                        }
                    }
                }
                NostrBech32::EventPointer(ep) => {
                    if GLOBALS.storage.read_event(ep.id)?.is_none() {
                        let relay_urls: Vec<RelayUrl> = ep
                            .relays
                            .iter()
                            .filter_map(|unchecked| {
                                RelayUrl::try_from_unchecked_url(unchecked).ok()
                            })
                            .collect();
                        let _ = GLOBALS
                            .to_overlord
                            .send(ToOverlordMessage::FetchEvent(ep.id, relay_urls));
                    }
                }
                NostrBech32::EventAddr(mut ea) => {
                    if let Ok(None) = GLOBALS
                        .storage
                        .get_replaceable_event(ea.kind, ea.author, &ea.d)
                    {
                        // Add the seen_on relay
                        if let Some(seen_on_url) = seen_on.as_ref() {
                            let seen_on_unchecked_url = seen_on_url.to_unchecked_url();
                            if !ea.relays.contains(&seen_on_unchecked_url) {
                                ea.relays.push(seen_on_unchecked_url);
                            }
                        }

                        let _ = GLOBALS
                            .to_overlord
                            .send(ToOverlordMessage::FetchEventAddr(ea));
                    }
                }
                NostrBech32::Profile(prof) => {
                    // Record existence of such a person
                    GLOBALS.people.create_if_missing(prof.pubkey);

                    // Make sure we have their relays
                    for relay in prof.relays {
                        if let Ok(rurl) = RelayUrl::try_from_unchecked_url(&relay) {
                            if let Some(_pr) =
                                GLOBALS.storage.read_person_relay(prof.pubkey, &rurl)?
                            {
                                // FIXME: we need a new field in PersonRelay for this case.
                                // If the event was signed by the profile person, we should trust it.
                                // If it wasn't, we can instead bump last_suggested_bytag.
                            } else {
                                let mut pr = PersonRelay::new(prof.pubkey, rurl);
                                pr.read = true;
                                pr.write = true;
                                GLOBALS.storage.write_person_relay(&pr, None)?;
                            }
                        }
                    }
                }
                NostrBech32::Pubkey(pubkey) => {
                    // Record existence of such a person
                    GLOBALS.people.create_if_missing(pubkey);
                }
                NostrBech32::Relay(relay) => {
                    if let Ok(rurl) = RelayUrl::try_from_unchecked_url(&relay) {
                        // make sure we have the relay
                        GLOBALS.storage.write_relay_if_missing(&rurl, None)?;
                    }
                }
            }
            // TBD: If the content contains an nprofile, make sure the pubkey is associated
            // with those relays
        }
    }

    // TBD (have to parse runes language for this)
    //if event.kind == EventKind::RelayList {
    //    process_somebody_elses_relay_list(event.pubkey.clone(), &event.contents).await?;
    //}

    // FIXME: Handle EventKind::RecommendedRelay

    Ok(())
}

async fn process_contact_list(event: &Event) -> Result<(), Error> {
    for tag in event.tags.iter() {
        if let Tag::Pubkey { pubkey, .. } = tag {
            let convert_pubkey = PublicKey::try_from(pubkey).ok();
            if let Some(to_pubkey) = convert_pubkey {
                tracing::debug!("---> PubKey {:?}", to_pubkey);
                GLOBALS.people.add_followed_person(event.pubkey, to_pubkey);
            }
        }
    }

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
        }
    } else {
        for tag in event.tags.iter() {
            tracing::debug!("Tag in event {:?}", tag);
            if let Tag::Pubkey { pubkey, .. } = tag {
                tracing::debug!("Contact List of {:?} - pubkey {:?}", event.pubkey, pubkey);
            }

            // put list in GLOBALS
        }
    }
    // We process the contents for (non-standard) relay list information.
    // Try to parse the contents as a SimpleRelayList (ignore if it is not)
    if let Ok(relay_list) = serde_json::from_str::<SimpleRelayList>(&event.content) {
        // Update that we received the relay list (and optionally bump forward the date
        // if this relay list happens to be newer)
        let newer = GLOBALS
            .people
            .update_relay_list_stamps(event.pubkey, event.created_at.0)
            .await?;

        if !newer {
            return Ok(());
        }

        let mut relay_list: RelayList = Default::default();
        for (url, simple_relay_usage) in srl.0.iter() {
            if let Ok(relay_url) = RelayUrl::try_from_unchecked_url(url) {
                if simple_relay_usage.read && simple_relay_usage.write {
                    relay_list.0.insert(relay_url, RelayUsage::Both);
                } else if simple_relay_usage.read {
                    relay_list.0.insert(relay_url, RelayUsage::Inbox);
                } else if simple_relay_usage.write {
                    relay_list.0.insert(relay_url, RelayUsage::Outbox);
                }
            }
        }
        GLOBALS
            .storage
            .set_relay_list(event.pubkey, relay_list, None)?;

        // the following also refreshes scores before it picks relays
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::RefreshScoresAndPickRelays);
    }

    Ok(())
}

/// Process relationships of an event.
/// This returns IDs that should be UI invalidated (must be redrawn)
pub(crate) fn process_relationships_of_event<'a>(
    event: &Event,
    rw_txn: Option<&mut RwTxn<'a>>,
) -> Result<Vec<Id>, Error> {
    let mut invalidate: Vec<Id> = Vec::new();

    let mut f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
        // replies to
        match event.replies_to() {
            Some(EventReference::Id { id, .. }) => {
                GLOBALS.storage.write_relationship_by_id(
                    id,
                    event.id,
                    RelationshipById::Reply,
                    Some(txn),
                )?;
            }
            Some(EventReference::Addr(ea)) => {
                GLOBALS.storage.write_relationship_by_addr(
                    ea,
                    event.id,
                    RelationshipByAddr::Reply,
                    Some(txn),
                )?;
            }
            None => (),
        }

        // timestamps
        if event.kind == EventKind::Timestamp {
            for tag in &event.tags {
                if let Ok((id, _, _)) = tag.parse_event() {
                    GLOBALS.storage.write_relationship_by_id(
                        id,
                        event.id,
                        RelationshipById::Timestamp,
                        Some(txn),
                    )?;
                }
            }
        }

        // deletes
        if let Some((vec, reason)) = event.deletes() {
            for er in vec.iter() {
                match er {
                    EventReference::Id { id, .. } => {
                        // If we have the event,
                        // Actually delete at this point in some cases
                        if let Some(deleted_event) = GLOBALS.storage.read_event(*id)? {
                            if !deleted_event.delete_author_allowed(event.pubkey) {
                                // No further processing if not a valid delete
                                continue;
                            }
                            invalidate.push(deleted_event.id);
                            if !deleted_event.kind.is_feed_displayable() {
                                // Otherwise actually delete (PITA to do otherwise)
                                GLOBALS.storage.delete_event(deleted_event.id, Some(txn))?;
                            }
                        }

                        // Store the delete (we either don't have the target to verify,
                        // or we just verified above. In the former case, it is okay because
                        // we verify on usage)
                        GLOBALS.storage.write_relationship_by_id(
                            *id,
                            event.id,
                            RelationshipById::Deletion {
                                by: event.pubkey,
                                reason: reason.clone(),
                            },
                            Some(txn),
                        )?;
                    }
                    EventReference::Addr(ea) => {
                        // If we have the event,
                        // Actually delete at this point in some cases
                        if let Some(deleted_event) = GLOBALS
                            .storage
                            .get_replaceable_event(ea.kind, ea.author, &ea.d)?
                        {
                            if !deleted_event.delete_author_allowed(event.pubkey) {
                                // No further processing if not a valid delete
                                continue;
                            }
                            invalidate.push(deleted_event.id);
                            if !deleted_event.kind.is_feed_displayable() {
                                // Otherwise actually delete (PITA to do otherwise)
                                GLOBALS.storage.delete_event(deleted_event.id, Some(txn))?;
                            }
                        }

                        // Store the delete (we either don't have the target to verify,
                        // or we just verified above. In the former case, it is okay because
                        // we verify on usage)
                        GLOBALS.storage.write_relationship_by_addr(
                            ea.clone(),
                            event.id,
                            RelationshipByAddr::Deletion {
                                by: event.pubkey,
                                reason: reason.clone(),
                            },
                            Some(txn),
                        )?;
                    }
                }
            }
        }

        // reacts to
        if let Some((reacted_to_id, reaction, _maybe_url)) = event.reacts_to() {
            // NOTE: reactions may precede the event they react to. So we cannot validate here.
            GLOBALS.storage.write_relationship_by_id(
                reacted_to_id, // event reacted to
                event.id,      // the reaction event id
                RelationshipById::Reaction {
                    by: event.pubkey,
                    reaction,
                },
                Some(txn),
            )?;
            invalidate.push(reacted_to_id);
        }

        // labels
        if event.kind == EventKind::Label {
            // Get the label from the "l" tag
            let mut label = "";
            let mut namespace = "";
            for t in &event.tags {
                if t.tagname() == "l" {
                    if t.value() != "" {
                        label = t.value();
                        if t.get_index(2) != "" {
                            namespace = t.get_index(2);
                        }
                    }
                }
            }

            for tag in &event.tags {
                if let Ok((id, _, _)) = tag.parse_event() {
                    GLOBALS.storage.write_relationship_by_id(
                        id,
                        event.id,
                        RelationshipById::Labels {
                            label: label.to_owned(),
                            namespace: namespace.to_owned(),
                        },
                        Some(txn),
                    )?;
                }
            }
        }

        // ListMutesThread
        if event.kind == EventKind::MuteList {
            for tag in &event.tags {
                if let Ok((id, _, _)) = tag.parse_event() {
                    GLOBALS.storage.write_relationship_by_id(
                        id,
                        event.id,
                        RelationshipById::ListMutesThread,
                        Some(txn),
                    )?;
                }
            }
        }

        // ListPins
        if event.kind == EventKind::PinList {
            for tag in &event.tags {
                if let Ok((id, _, _)) = tag.parse_event() {
                    GLOBALS.storage.write_relationship_by_id(
                        id,
                        event.id,
                        RelationshipById::ListPins,
                        Some(txn),
                    )?;
                }
            }
        }

        // ListBookmarks
        if event.kind == EventKind::BookmarkList {
            for tag in &event.tags {
                if let Ok((id, _, _)) = tag.parse_event() {
                    GLOBALS.storage.write_relationship_by_id(
                        id,
                        event.id,
                        RelationshipById::ListBookmarks,
                        Some(txn),
                    )?;
                }

                if let Ok((ea, _marker)) = tag.parse_address() {
                    GLOBALS.storage.write_relationship_by_addr(
                        ea,
                        event.id,
                        RelationshipByAddr::ListBookmarks,
                        Some(txn),
                    )?;
                }
            }
        }

        // BookmarkSets
        if event.kind == EventKind::BookmarkSets {
            for tag in &event.tags {
                if let Ok((id, _, _)) = tag.parse_event() {
                    GLOBALS.storage.write_relationship_by_id(
                        id,
                        event.id,
                        RelationshipById::ListBookmarks,
                        Some(txn),
                    )?;
                }

                if let Ok((ea, _marker)) = tag.parse_address() {
                    GLOBALS.storage.write_relationship_by_addr(
                        ea,
                        event.id,
                        RelationshipByAddr::ListBookmarks,
                        Some(txn),
                    )?;
                }
            }
        }

        // CurationSets
        if event.kind == EventKind::CurationSets {
            for tag in &event.tags {
                if let Ok((id, _, _)) = tag.parse_event() {
                    GLOBALS.storage.write_relationship_by_id(
                        id,
                        event.id,
                        RelationshipById::Curation,
                        Some(txn),
                    )?;
                }
                if let Ok((ea, _marker)) = tag.parse_address() {
                    GLOBALS.storage.write_relationship_by_addr(
                        ea,
                        event.id,
                        RelationshipByAddr::Curation,
                        Some(txn),
                    )?;
                }
            }
        }

        if event.kind == EventKind::LiveChatMessage {
            for tag in &event.tags {
                if let Ok((ea, _marker)) = tag.parse_address() {
                    GLOBALS.storage.write_relationship_by_addr(
                        ea,
                        event.id,
                        RelationshipByAddr::LiveChatMessage,
                        Some(txn),
                    )?;
                }
            }
        }

        if event.kind == EventKind::BadgeAward {
            for tag in &event.tags {
                if let Ok((ea, _marker)) = tag.parse_address() {
                    GLOBALS.storage.write_relationship_by_addr(
                        ea,
                        event.id,
                        RelationshipByAddr::BadgeAward,
                        Some(txn),
                    )?;
                }
            }
        }

        if event.kind == EventKind::HandlerRecommendation {
            for tag in &event.tags {
                if let Ok((ea, _marker)) = tag.parse_address() {
                    GLOBALS.storage.write_relationship_by_addr(
                        ea,
                        event.id,
                        RelationshipByAddr::HandlerRecommendation,
                        Some(txn),
                    )?;
                }
            }
        }

        if event.kind == EventKind::Reporting {
            for tag in &event.tags {
                if let Ok((id, Some(rurl), _)) = tag.parse_event() {
                    let report = &rurl.0;
                    GLOBALS.storage.write_relationship_by_id(
                        id,
                        event.id,
                        RelationshipById::Reports(report.to_owned()),
                        Some(txn),
                    )?;
                }
            }
        }

        // zaps
        match event.zaps() {
            Ok(Some(zapdata)) => {
                GLOBALS.storage.write_relationship_by_id(
                    zapdata.id,
                    event.id,
                    RelationshipById::ZapReceipt {
                        by: event.pubkey,
                        amount: zapdata.amount,
                    },
                    Some(txn),
                )?;

                invalidate.push(zapdata.id);
            }
            Err(e) => tracing::error!("Invalid zap receipt: {}", e),
            _ => {}
        }

        // JobResult
        if event.kind.is_job_result() {
            for tag in &event.tags {
                if let Ok((id, _, _)) = tag.parse_event() {
                    GLOBALS.storage.write_relationship_by_id(
                        id,
                        event.id,
                        RelationshipById::JobResult,
                        Some(txn),
                    )?;
                }
            }
        }

        Ok(())
    };

    match rw_txn {
        Some(txn) => f(txn)?,
        None => {
            let mut txn = GLOBALS.storage.get_write_txn()?;
            f(&mut txn)?;
            txn.commit()?;
        }
    };

    Ok(invalidate)
}

// This updates the event data and maybe the title, but it does NOT update the list
// (that happens only when the user overwrites/merges)
fn update_or_allocate_person_list_from_event(
    event: &Event,
    pubkey: PublicKey,
) -> Result<(PersonList, PersonListMetadata), Error> {
    // Determine PersonList and fetch Metadata
    let (list, mut metadata, new) = crate::people::fetch_current_personlist_matching_event(event)?;

    // Update metadata
    {
        metadata.event_created_at = event.created_at;

        metadata.event_public_len = event.tags.iter().filter(|t| t.tagname() == "p").count();

        if event.kind == EventKind::ContactList {
            metadata.event_private_len = None;
        } else if GLOBALS.identity.is_unlocked() {
            let mut private_len: Option<usize> = None;
            if let Ok(bytes) = GLOBALS.identity.decrypt(&pubkey, &event.content) {
                if let Ok(vectags) = serde_json::from_str::<Vec<Tag>>(&bytes) {
                    private_len = Some(vectags.iter().filter(|t| t.tagname() == "p").count());
                }
            }
            metadata.event_private_len = private_len;
        }

        if let Some(title) = event.title() {
            metadata.title = title.to_owned();
        }

        // If title is empty, use the d-tag
        if metadata.title.is_empty() && !metadata.dtag.is_empty() {
            metadata.title = metadata.dtag.clone();
        }
    }

    // Save metadata
    GLOBALS
        .storage
        .set_person_list_metadata(list, &metadata, None)?;

    if new {
        // Ask the overlord to populate the list from the event, since it is
        // locally new
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::UpdatePersonList {
                person_list: list,
                merge: false,
            });
    }

    Ok((list, metadata))
}
