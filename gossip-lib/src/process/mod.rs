use crate::bookmarks::BookmarkList;
use crate::comms::ToOverlordMessage;
use crate::error::Error;
use crate::globals::GLOBALS;
use crate::misc::Private;
use crate::relationship::{RelationshipByAddr, RelationshipById};
use crate::storage::{PersonTable, Table};
use crate::Relay;
use heed::RwTxn;
use nostr_types::{
    Event, EventKind, EventReference, Filter, Id, NAddr, NostrBech32, RelayUrl, Unixtime,
};
use std::sync::atomic::Ordering;

mod by_kind;

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
    // Now
    let now = Unixtime::now();

    // Determine if this came in on global
    let global_feed = match subscription {
        Some(ref s) => s.contains("global_feed"),
        _ => false,
    };

    // Bump count of events processed
    GLOBALS.events_processed.fetch_add(1, Ordering::SeqCst);

    // Detect if duplicate.
    // We still need to process some things even if a duplicate
    let duplicate = if global_feed {
        GLOBALS.db().has_volatile_event(event.id)
    } else {
        GLOBALS.db().has_event(event.id)?
    };

    // Verify the event,
    // Don't verify if it is a duplicate:
    //    NOTE: relays could send forged events with valid IDs of other events, but if
    //          they do that in an event that is a duplicate of one we already have, this
    //          duplicate will only affect seen-on information, it will not be saved.
    if !duplicate && verify {
        let mut maxtime = now;
        maxtime.0 += GLOBALS.db().read_setting_future_allowance_secs() as i64;
        if let Err(e) = event.verify(Some(maxtime)) {
            // Don't print these, they clutter the console
            tracing::debug!("{}: VERIFY ERROR: {}", e, serde_json::to_string(&event)?);
            return Ok(());
        }
    }

    // Create the person if missing in the database
    PersonTable::create_record_if_missing(event.pubkey, None)?;

    // Update seen_on relay related information
    let mut spamsafe = false;
    if let Some(url) = &seen_on {
        // Save seen-on-relay information
        if global_feed {
            GLOBALS
                .db()
                .add_event_seen_on_relay_volatile(event.id, url.to_owned(), now);
        } else {
            GLOBALS
                .db()
                .add_event_seen_on_relay(event.id, url, now, None)?;
        }

        // Update person-relay information (seen them on this relay)
        GLOBALS.db().modify_person_relay(
            event.pubkey,
            url,
            |pr| pr.last_fetched = Some(now.0 as u64),
            None,
        )?;

        if let Some(relay) = GLOBALS.db().read_relay(url)? {
            spamsafe = relay.has_usage_bits(Relay::SPAMSAFE);
        }
    }

    // Process with spam filter
    if !global_feed
        && GLOBALS
            .db()
            .read_setting_apply_spam_filter_on_incoming_events()
    {
        use crate::spam_filter::{EventFilterAction, EventFilterCaller};
        let filter_result =
            crate::spam_filter::filter_event(event.clone(), EventFilterCaller::Process, spamsafe);
        match filter_result {
            EventFilterAction::Allow => {}
            EventFilterAction::Deny => return Ok(()),
            EventFilterAction::MuteAuthor => {
                GLOBALS.people.mute(&event.pubkey, true, Private(false))?;
                return Ok(());
            }
        }
    }

    // Invalidate the note itself (due to seen_on probably changing)
    GLOBALS.ui_notes_to_invalidate.write().push(event.id);

    // Bail out if duplicate (in most cases)
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

    // Bail out if the event was deleted (by id)
    for (_id, relbyid) in GLOBALS.db().find_relationships_by_id(event.id)? {
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

    // Bail out if the event was deleted (by address)
    if let Some(parameter) = event.parameter() {
        let ea = NAddr {
            d: parameter.to_owned(),
            relays: vec![],
            kind: event.kind,
            author: event.pubkey,
        };
        for (_id, relbyaddr) in GLOBALS.db().find_relationships_by_addr(&ea)? {
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
    if global_feed {
        GLOBALS.db().write_event_volatile(event.to_owned());
    } else if event.kind.is_replaceable() {
        // Bail if the event is an already-replaced replaceable event
        if !GLOBALS.db().replace_event(event, None)? {
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
        GLOBALS.db().write_event(event, None)?;
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

    // Create referenced relays and people, and update person_relay associations
    if seen_on.is_some() {
        for tag in event.tags.iter() {
            if let Ok((_, Some(uurl), _optmarker, _optpubkey)) = tag.parse_event() {
                if let Ok(url) = RelayUrl::try_from_unchecked_url(&uurl) {
                    GLOBALS.db().write_relay_if_missing(&url, None)?;
                }
            }

            if let Ok((pubkey, maybeurl, _)) = tag.parse_pubkey() {
                PersonTable::create_record_if_missing(pubkey, None)?;

                if let Some(uncheckedurl) = maybeurl {
                    if let Ok(url) = RelayUrl::try_from_unchecked_url(&uncheckedurl) {
                        GLOBALS.db().write_relay_if_missing(&url, None)?;

                        // upsert person_relay.last_suggested
                        GLOBALS.db().modify_person_relay(
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

    // Let seeker know about this event id (in case it was sought)
    GLOBALS.seeker.found(event)?;

    // Save event relationships (whether from a relay or not)
    // and invalidate UI events that need to be redrawn because those relationships
    // affect their rendering.
    let invalid_ids = process_relationships_of_event(event, None)?;
    GLOBALS.ui_notes_to_invalidate.write().extend(&invalid_ids);

    if event.kind.is_feed_displayable() {
        process_feed_displayable_content(event, seen_on.as_ref(), now)?;
    }

    let mut ours: bool = false;
    if let Some(pubkey) = GLOBALS.identity.public_key() {
        if event.pubkey == pubkey {
            ours = true;
        }
    }

    match event.kind {
        EventKind::Metadata => by_kind::process_metadata(event)?,
        EventKind::HandlerRecommendation => by_kind::process_handler_recommendation(event)?,
        EventKind::HandlerInformation => by_kind::process_handler_information(event)?,
        EventKind::ContactList => by_kind::process_contact_list(event)?,
        EventKind::MuteList => by_kind::process_mute_list(event, ours)?,
        EventKind::FollowSets => by_kind::process_follow_sets(event, ours)?,
        EventKind::RelayList => by_kind::process_relay_list(event)?,
        EventKind::DmRelayList => by_kind::process_dm_relay_list(event)?,
        EventKind::Repost => by_kind::process_repost(event, verify)?,
        EventKind::NostrConnect => by_kind::process_nostr_connect(event, seen_on.clone())?,
        EventKind::UserServerList => by_kind::process_user_server_list(event, ours)?,
        _ => {}
    }

    Ok(())
}

// Process the content for references to things we might want
fn process_feed_displayable_content(
    event: &Event,
    seen_on: Option<&RelayUrl>,
    now: Unixtime,
) -> Result<(), Error> {
    for bech32 in NostrBech32::find_all_in_string(&event.content) {
        match bech32 {
            NostrBech32::CryptSec(_) => {
                // do nothing here
            }
            NostrBech32::Id(id) => {
                if GLOBALS.db().read_event(id)?.is_none() {
                    if let Some(relay_url) = seen_on {
                        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FetchEvent(
                            id,
                            vec![relay_url.to_owned()],
                        ));
                    }
                }
            }
            NostrBech32::NEvent(ne) => {
                if GLOBALS.db().read_event(ne.id)?.is_none() {
                    let relay_urls: Vec<RelayUrl> = ne
                        .relays
                        .iter()
                        .filter_map(|unchecked| RelayUrl::try_from_unchecked_url(unchecked).ok())
                        .collect();
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::FetchEvent(ne.id, relay_urls));
                }
            }
            NostrBech32::NAddr(mut ea) => {
                if let Ok(None) = GLOBALS
                    .db()
                    .get_replaceable_event(ea.kind, ea.author, &ea.d)
                {
                    // Add the seen_on relay
                    if let Some(seen_on_url) = seen_on {
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
                        GLOBALS.db().modify_person_relay(
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
                    GLOBALS.db().write_relay_if_missing(&rurl, None)?;
                }
            }
        }
        // TBD: If the content contains an nprofile, make sure the pubkey is associated
        // with those relays
    }

    Ok(())
}

/// Process relationships of an event.
/// This returns IDs that should be UI invalidated (must be redrawn)
pub(crate) fn process_relationships_of_event(
    event: &Event,
    rw_txn: Option<&mut RwTxn<'_>>,
) -> Result<Vec<Id>, Error> {
    let mut invalidate: Vec<Id> = Vec::new();

    let mut local_txn = None;
    let txn = match rw_txn {
        Some(x) => x,
        None => {
            local_txn = Some(GLOBALS.db().get_write_txn()?);
            local_txn.as_mut().unwrap()
        }
    };

    // timestamps
    if event.kind == EventKind::Timestamp {
        for tag in &event.tags {
            if let Ok((id, _, _, _)) = tag.parse_event() {
                GLOBALS.db().write_relationship_by_id(
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
                    if let Some(deleted_event) = GLOBALS.db().read_event(*id)? {
                        if !deleted_event.delete_author_allowed(event.pubkey) {
                            // No further processing if not a valid delete
                            continue;
                        }
                        invalidate.push(deleted_event.id);
                        if !deleted_event.kind.is_feed_displayable() {
                            // Otherwise actually delete (PITA to do otherwise)
                            GLOBALS.db().delete_event(deleted_event.id, Some(txn))?;
                        }
                    }

                    // Store the delete (we either don't have the target to verify,
                    // or we just verified above. In the former case, it is okay because
                    // we verify on usage)
                    GLOBALS.db().write_relationship_by_id(
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
                        .db()
                        .get_replaceable_event(ea.kind, ea.author, &ea.d)?
                    {
                        if !deleted_event.delete_author_allowed(event.pubkey) {
                            // No further processing if not a valid delete
                            continue;
                        }
                        invalidate.push(deleted_event.id);
                        if !deleted_event.kind.is_feed_displayable() {
                            // Otherwise actually delete (PITA to do otherwise)
                            GLOBALS.db().delete_event(deleted_event.id, Some(txn))?;
                        }
                    }

                    // Store the delete (we either don't have the target to verify,
                    // or we just verified above. In the former case, it is okay because
                    // we verify on usage)
                    GLOBALS.db().write_relationship_by_addr(
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

    if let Some((
        EventReference::Id {
            id: reacted_to_id, ..
        },
        reaction,
    )) = event.reacts_to()
    {
        // NOTE: reactions may precede the event they react to. So we cannot validate here.
        GLOBALS.db().write_relationship_by_id(
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
            if let Ok((id, _, _, _)) = tag.parse_event() {
                GLOBALS.db().write_relationship_by_id(
                    id,
                    event.id,
                    RelationshipById::Labels {
                        label: label.to_owned(),
                        namespace: namespace.to_owned(),
                    },
                    Some(txn),
                )?;
            } else if let Ok((ea, _marker)) = tag.parse_address() {
                GLOBALS.db().write_relationship_by_addr(
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
            if let Ok((id, _, _, _)) = tag.parse_event() {
                GLOBALS.db().write_relationship_by_id(
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
            if let Ok((id, _, _, _)) = tag.parse_event() {
                GLOBALS.db().write_relationship_by_id(
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
                        .db()
                        .get_replaceable_event(EventKind::BookmarkList, pk, "")?
                {
                    if newest_event == *event {
                        *GLOBALS.bookmarks.write_arc() = BookmarkList::from_event(event)?;
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
                GLOBALS.db().write_relationship_by_addr(
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
                GLOBALS.db().write_relationship_by_addr(
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
                GLOBALS.db().write_relationship_by_addr(
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
            if let Ok((id, Some(rurl), _, _)) = tag.parse_event() {
                let report = &rurl.0;
                GLOBALS.db().write_relationship_by_id(
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
        match zapdata.zapped_event {
            EventReference::Id { id, .. } => {
                GLOBALS.db().write_relationship_by_id(
                    id,
                    event.id,
                    RelationshipById::Zaps {
                        by: zapdata.payer,
                        amount: zapdata.amount,
                    },
                    Some(txn),
                )?;
                invalidate.push(id);
            }
            EventReference::Addr(naddr) => {
                GLOBALS.db().write_relationship_by_addr(
                    naddr,
                    event.id,
                    RelationshipByAddr::Zaps {
                        by: zapdata.payer,
                        amount: zapdata.amount,
                    },
                    Some(txn),
                )?;
            }
        }
    }

    // JobResult
    if event.kind.is_job_result() {
        for tag in &event.tags {
            if let Ok((id, _, _, _)) = tag.parse_event() {
                GLOBALS.db().write_relationship_by_id(
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
            GLOBALS.db().write_relationship_by_id(
                inner_event.id,
                event.id,
                RelationshipById::Reposts,
                Some(txn),
            )?;
        } else {
            for eref in event.mentions().iter() {
                if let EventReference::Id { id, .. } = eref {
                    GLOBALS.db().write_relationship_by_id(
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
            GLOBALS.db().write_relationship_by_id(
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
                GLOBALS.db().write_relationship_by_id(
                    id,
                    event.id,
                    RelationshipById::Annotates,
                    Some(txn),
                )?;
                invalidate.push(id);
            } else {
                GLOBALS.db().write_relationship_by_id(
                    id,
                    event.id,
                    RelationshipById::RepliesTo,
                    Some(txn),
                )?;
            }
        }
        Some(EventReference::Addr(ea)) => {
            if event.is_annotation() {
                GLOBALS.db().write_relationship_by_addr(
                    ea,
                    event.id,
                    RelationshipByAddr::Annotates,
                    Some(txn),
                )?;
            } else {
                GLOBALS.db().write_relationship_by_addr(
                    ea,
                    event.id,
                    RelationshipByAddr::RepliesTo,
                    Some(txn),
                )?;
            }
        }
        None => (),
    }

    if let Some(txn) = local_txn {
        txn.commit()?;
    }

    Ok(invalidate)
}

pub fn reprocess_relay_lists() -> Result<(usize, usize), Error> {
    let mut counts: (usize, usize) = (0, 0);

    // Reprocess all contact lists
    let mut filter = Filter::new();
    filter.add_event_kind(EventKind::ContactList);
    let events = GLOBALS.db().find_events_by_filter(&filter, |_e| true)?;
    for event in &events {
        by_kind::process_somebody_elses_contact_list(event, true)?;
    }
    counts.0 = events.len();

    // Reprocess all relay lists
    let mut filter = Filter::new();
    filter.add_event_kind(EventKind::RelayList);

    let mut txn = GLOBALS.db().get_write_txn()?;
    let relay_lists = GLOBALS.db().find_events_by_filter(&filter, |_| true)?;

    // Process all RelayLists
    for event in relay_lists.iter() {
        GLOBALS
            .db()
            .process_relay_list(event, true, Some(&mut txn))?;
    }
    counts.1 = events.len();

    // Turn off the flag
    GLOBALS
        .db()
        .set_flag_reprocess_relay_lists_needed(false, Some(&mut txn))?;

    txn.commit()?;

    Ok(counts)
}
