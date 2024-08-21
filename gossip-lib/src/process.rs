use crate::bookmarks::BookmarkList;
use crate::comms::ToOverlordMessage;
use crate::error::Error;
use crate::filter::EventFilterAction;
use crate::globals::GLOBALS;
use crate::misc::{Freshness, Private};
use crate::people::{People, PersonList, PersonListMetadata};
use crate::relationship::{RelationshipByAddr, RelationshipById};
use crate::storage::{PersonTable, Table};
use heed::RwTxn;
use nostr_types::{
    Event, EventKind, EventReference, Filter, Id, Metadata, NAddr, NostrBech32, PublicKey,
    RelayList, RelayListUsage, RelayUrl, SimpleRelayList, Tag, Unixtime,
};
use std::sync::atomic::Ordering;

/// This is mainly used internally to gossip-lib, but you can use it to stuff events
/// into gossip from other sources. This processes a new event, saving the results into
/// the database and also populating the GLOBALS maps.
pub fn process_new_event(
    event: &Event,
    seen_on: Option<RelayUrl>,
    subscription: Option<String>,
    verify: bool,
    process_even_if_duplicate: bool,
) -> Result<(), Error> {
    let now = Unixtime::now();

    let global_feed = match subscription {
        Some(ref s) => s == "global_feed",
        _ => false,
    };

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
            tracing::warn!("{}: VERIFY ERROR: {}", e, serde_json::to_string(&event)?);
            return Ok(());
        }
    }

    if let Some(url) = &seen_on {
        // Save seen-on-relay information
        if global_feed {
            GLOBALS
                .storage
                .add_event_seen_on_relay_volatile(event.id, url.to_owned(), now);
        } else {
            GLOBALS
                .storage
                .add_event_seen_on_relay(event.id, url, now, None)?;
        }

        // Create the person if missing in the database
        PersonTable::create_record_if_missing(event.pubkey, None)?;

        // Update person-relay information (seen them on this relay)
        GLOBALS.storage.modify_person_relay(
            event.pubkey,
            url,
            |pr| pr.last_fetched = Some(now.0 as u64),
            None,
        )?;
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
                    let author = PersonTable::read_record(rumor.pubkey, None)?;
                    Some(crate::filter::filter_rumor(rumor, author, event.id))
                } else {
                    None
                }
            } else {
                let author = PersonTable::read_record(event.pubkey, None)?;
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
                GLOBALS.people.mute(&event.pubkey, true, Private(false))?;
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
        if let RelationshipById::Deletes { by, reason: _ } = relbyid {
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
        let ea = NAddr {
            d: parameter.to_owned(),
            relays: vec![],
            kind: event.kind,
            author: event.pubkey,
        };
        for (_id, relbyaddr) in GLOBALS.storage.find_relationships_by_addr(&ea)? {
            if let RelationshipByAddr::Deletes { by, reason: _ } = relbyaddr {
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
    if event.kind.is_replaceable() && !global_feed {
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
    } else if global_feed {
        GLOBALS.storage.write_event_volatile(event.to_owned());
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
        if let Ok(rumor) = GLOBALS.identity.unwrap_giftwrap(event) {
            rumor_event = rumor.into_event_with_bad_signature();
            rumor_event.id = event.id; // Lie so it's handled with the giftwrap's id
            event = &rumor_event;
        } else {
            // Not for us.
            return Ok(());
        }
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

                        // upsert person_relay.last_suggested
                        GLOBALS.storage.modify_person_relay(
                            pubkey,
                            &url,
                            |pr| pr.last_suggested = Some(now.0 as u64),
                            None,
                        )?;
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
    GLOBALS.seeker.found(event)?;

    // If metadata, update person
    if event.kind == EventKind::Metadata {
        let metadata: Metadata = serde_json::from_str(&event.content)?;

        GLOBALS
            .people
            .update_metadata(&event.pubkey, metadata, event.created_at)?;
    }

    if event.kind == EventKind::ContactList {
        if let Some(pubkey) = GLOBALS.identity.public_key() {
            if event.pubkey == pubkey {
                // Updates stamps and counts, does NOT change membership
                let (_personlist, _metadata) =
                    update_or_allocate_person_list_from_event(event, pubkey)?;
            } else {
                process_somebody_elses_contact_list(event, false)?;
            }
        } else {
            process_somebody_elses_contact_list(event, false)?;
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
        GLOBALS.storage.process_relay_list(event, false, None)?;

        // Let the seeker know we now have relays for this author, in case the seeker
        // wants to update it's state
        // (we might not, but by this point we have tried)
        GLOBALS.seeker.found_author_relays(event.pubkey);

        // the following also refreshes scores before it picks relays
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::RefreshScoresAndPickRelays);
    } else if event.kind == EventKind::DmRelayList {
        GLOBALS.storage.process_dm_relay_list(event, None)?;
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
            process_new_event(&inner_event, None, None, verify, false)?;

            // Seek additional info for this event by id and author
            GLOBALS
                .seeker
                .seek_id_and_author(inner_event.id, inner_event.pubkey, vec![], false)?;
        } else {
            // If the content is a repost, seek the event it reposts
            for eref in event.mentions().iter() {
                match eref {
                    EventReference::Id { id, relays, .. } => {
                        if relays.is_empty() {
                            // Even if the event tags the author, we have no way to coorelate
                            // the nevent with that tag.
                            GLOBALS.seeker.seek_id(*id, vec![], false)?;
                        } else {
                            GLOBALS
                                .seeker
                                .seek_id_and_relays(*id, relays.clone(), false);
                        }
                    }
                    EventReference::Addr(ea) => {
                        if !ea.relays.is_empty() {
                            let _ = GLOBALS
                                .to_overlord
                                .send(ToOverlordMessage::FetchNAddr(ea.clone()));
                        }
                    }
                }
            }
        }
    } else if event.kind == EventKind::NostrConnect {
        crate::nostr_connect_server::handle_command(event, seen_on.clone())?
    }

    if event.kind.is_feed_displayable() {
        // Process the content for references to things we might want
        for bech32 in NostrBech32::find_all_in_string(&event.content) {
            match bech32 {
                NostrBech32::CryptSec(_) => {
                    // do nothing here
                }
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
                NostrBech32::NEvent(ne) => {
                    if GLOBALS.storage.read_event(ne.id)?.is_none() {
                        let relay_urls: Vec<RelayUrl> = ne
                            .relays
                            .iter()
                            .filter_map(|unchecked| {
                                RelayUrl::try_from_unchecked_url(unchecked).ok()
                            })
                            .collect();
                        let _ = GLOBALS
                            .to_overlord
                            .send(ToOverlordMessage::FetchEvent(ne.id, relay_urls));
                    }
                }
                NostrBech32::NAddr(mut ea) => {
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

                        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FetchNAddr(ea));
                    }
                }
                NostrBech32::Profile(prof) => {
                    // Record existence of such a person
                    GLOBALS.people.create_if_missing(prof.pubkey);

                    // Make sure we have their relays
                    for relay in prof.relays {
                        if let Ok(rurl) = RelayUrl::try_from_unchecked_url(&relay) {
                            GLOBALS.storage.modify_person_relay(
                                prof.pubkey,
                                &rurl,
                                |pr| {
                                    if prof.pubkey == event.pubkey {
                                        // The author themselves said it
                                        pr.read = true;
                                        pr.write = true;
                                    } else {
                                        // It was suggested by someone else
                                        pr.last_suggested = Some(now.0 as u64);
                                    }
                                },
                                None,
                            )?
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
    //    process_somebody_elses_relay_list(event.pubkey.clone(), &event.contents)?;
    //}

    // FIXME: Handle EventKind::RecommendedRelay

    Ok(())
}

fn process_somebody_elses_contact_list(event: &Event, force: bool) -> Result<(), Error> {
    // We don't keep their contacts or show to the user yet.
    // We only process the contents for (non-standard) relay list information.

    // Try to parse the contents as a SimpleRelayList (ignore if it is not)
    if let Ok(srl) = serde_json::from_str::<SimpleRelayList>(&event.content) {
        // Update that we received the relay list (and optionally bump forward the date
        // if this relay list happens to be newer)
        let newer = GLOBALS
            .people
            .update_relay_list_stamps(event.pubkey, event.created_at.0)?;

        if !newer && !force {
            return Ok(());
        }

        let mut relay_list: RelayList = Default::default();
        for (url, simple_relay_usage) in srl.0.iter() {
            if let Ok(relay_url) = RelayUrl::try_from_unchecked_url(url) {
                if simple_relay_usage.read && simple_relay_usage.write {
                    relay_list.0.insert(relay_url, RelayListUsage::Both);
                } else if simple_relay_usage.read {
                    relay_list.0.insert(relay_url, RelayListUsage::Inbox);
                } else if simple_relay_usage.write {
                    relay_list.0.insert(relay_url, RelayListUsage::Outbox);
                }
            }
        }
        GLOBALS
            .storage
            .set_relay_list(event.pubkey, relay_list, None)?;

        if !force {
            // the following also refreshes scores before it picks relays
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::RefreshScoresAndPickRelays);
        }
    } else if !event.content.is_empty() {
        tracing::info!("Contact list content does not parse: {}", &event.content);
    }

    Ok(())
}

pub fn reprocess_relay_lists() -> Result<(usize, usize), Error> {
    let mut counts: (usize, usize) = (0, 0);

    // Reprocess all contact lists
    let mut filter = Filter::new();
    filter.add_event_kind(EventKind::ContactList);
    let events = GLOBALS.storage.find_events_by_filter(&filter, |_e| true)?;
    for event in &events {
        process_somebody_elses_contact_list(event, true)?;
    }
    counts.0 = events.len();

    // Reprocess all relay lists
    let mut filter = Filter::new();
    filter.add_event_kind(EventKind::RelayList);

    let mut txn = GLOBALS.storage.get_write_txn()?;
    let relay_lists = GLOBALS.storage.find_events_by_filter(&filter, |_| true)?;

    // Process all RelayLists
    for event in relay_lists.iter() {
        GLOBALS
            .storage
            .process_relay_list(event, true, Some(&mut txn))?;
    }
    counts.1 = events.len();

    // Turn off the flag
    GLOBALS
        .storage
        .set_flag_reprocess_relay_lists_needed(false, Some(&mut txn))?;

    txn.commit()?;

    Ok(counts)
}

/// Process relationships of an event.
/// This returns IDs that should be UI invalidated (must be redrawn)
pub(crate) fn process_relationships_of_event<'a>(
    event: &Event,
    rw_txn: Option<&mut RwTxn<'a>>,
) -> Result<Vec<Id>, Error> {
    let mut invalidate: Vec<Id> = Vec::new();

    let mut f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
        // timestamps
        if event.kind == EventKind::Timestamp {
            for tag in &event.tags {
                if let Ok((id, _, _)) = tag.parse_event() {
                    GLOBALS.storage.write_relationship_by_id(
                        id,
                        event.id,
                        RelationshipById::Timestamps,
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
                            RelationshipById::Deletes {
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
                            RelationshipByAddr::Deletes {
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
                RelationshipById::ReactsTo {
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
                } else if let Ok((ea, _marker)) = tag.parse_address() {
                    GLOBALS.storage.write_relationship_by_addr(
                        ea,
                        event.id,
                        RelationshipByAddr::Labels {
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
                        RelationshipById::Mutes,
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
                        RelationshipById::Pins,
                        Some(txn),
                    )?;
                }
            }
        }

        // Maybe update global's cache of bookmarks
        if event.kind == EventKind::BookmarkList {
            // Only if it is ours
            if let Some(pk) = GLOBALS.identity.public_key() {
                if pk == event.pubkey {
                    // Only if this event is the latest (it is already stored so we can do this check)
                    if let Some(newest_event) =
                        GLOBALS
                            .storage
                            .get_replaceable_event(EventKind::BookmarkList, pk, "")?
                    {
                        if newest_event == *event {
                            *GLOBALS.bookmarks.write() = BookmarkList::from_event(event)?;
                            GLOBALS.recompute_current_bookmarks.notify_one();
                        }
                    }
                }
            }
        }

        // NOTE: we do not store Bookmarks or Curates relationships anymore.

        if event.kind == EventKind::LiveChatMessage {
            for tag in &event.tags {
                if let Ok((ea, _marker)) = tag.parse_address() {
                    GLOBALS.storage.write_relationship_by_addr(
                        ea,
                        event.id,
                        RelationshipByAddr::ChatsWithin,
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
                        RelationshipByAddr::AwardsBadge,
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
                        RelationshipByAddr::RecommendsHandler,
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
        if let Ok(Some(zapdata)) = event.zaps() {
            GLOBALS.storage.write_relationship_by_id(
                zapdata.id,
                event.id,
                RelationshipById::Zaps {
                    by: event.pubkey,
                    amount: zapdata.amount,
                },
                Some(txn),
            )?;

            invalidate.push(zapdata.id);
        }

        // JobResult
        if event.kind.is_job_result() {
            for tag in &event.tags {
                if let Ok((id, _, _)) = tag.parse_event() {
                    GLOBALS.storage.write_relationship_by_id(
                        id,
                        event.id,
                        RelationshipById::SuppliesJobResult,
                        Some(txn),
                    )?;
                }
            }
        }

        // Reposts
        if event.kind == EventKind::Repost {
            if let Ok(inner_event) = serde_json::from_str::<Event>(&event.content) {
                GLOBALS.storage.write_relationship_by_id(
                    inner_event.id,
                    event.id,
                    RelationshipById::Reposts,
                    Some(txn),
                )?;
            } else {
                for eref in event.mentions().iter() {
                    if let EventReference::Id { id, .. } = eref {
                        GLOBALS.storage.write_relationship_by_id(
                            *id,
                            event.id,
                            RelationshipById::Reposts,
                            Some(txn),
                        )?;
                    }
                }
            }
        }

        // Quotes
        for eref in event.quotes().iter() {
            if let EventReference::Id { id, .. } = eref {
                GLOBALS.storage.write_relationship_by_id(
                    *id,
                    event.id,
                    RelationshipById::Quotes,
                    Some(txn),
                )?;
            }
        }

        // RepliesTo (or Annotation)
        match event.replies_to() {
            Some(EventReference::Id { id, .. }) => {
                if event.is_annotation() {
                    GLOBALS.storage.write_relationship_by_id(
                        id,
                        event.id,
                        RelationshipById::Annotates,
                        Some(txn),
                    )?;
                    invalidate.push(id);
                } else {
                    GLOBALS.storage.write_relationship_by_id(
                        id,
                        event.id,
                        RelationshipById::RepliesTo,
                        Some(txn),
                    )?;
                }
            }
            Some(EventReference::Addr(ea)) => {
                if event.is_annotation() {
                    GLOBALS.storage.write_relationship_by_addr(
                        ea,
                        event.id,
                        RelationshipByAddr::Annotates,
                        Some(txn),
                    )?;
                } else {
                    GLOBALS.storage.write_relationship_by_addr(
                        ea,
                        event.id,
                        RelationshipByAddr::RepliesTo,
                        Some(txn),
                    )?;
                }
            }
            None => (),
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
