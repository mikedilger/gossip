use crate::comms::ToOverlordMessage;
use crate::error::{Error, ErrorKind};
use crate::filter::EventFilterAction;
use crate::globals::GLOBALS;
use crate::people::{PersonList, PersonListMetadata};
use crate::person_relay::PersonRelay;
use crate::relationship::{RelationshipByAddr, RelationshipById};
use async_recursion::async_recursion;
use heed::RwTxn;
use nostr_types::{
    Event, EventAddr, EventKind, EventReference, Id, Metadata, NostrBech32, PublicKey, RelayUrl,
    SimpleRelayList, Tag, Unixtime,
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
    if event.kind.is_feed_displayable()
        && !GLOBALS
            .people
            .is_person_in_list(&event.pubkey, PersonList::Followed)
    {
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
                let public = true;
                GLOBALS.people.mute(&event.pubkey, true, public)?;
                return Ok(());
            }
        }
    }

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

    // Save event relationships (whether from a relay or not)
    let invalid_ids = process_relationships_of_event(event, None)?;

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
                // Update this data for the UI.  We don't actually process the latest event
                // until the user gives the go ahead.
                GLOBALS.people.update_latest_person_list_event_data();
            } else {
                process_somebody_elses_contact_list(event).await?;
            }
        } else {
            process_somebody_elses_contact_list(event).await?;
        }
    } else if event.kind == EventKind::MuteList || event.kind == EventKind::FollowSets {
        // Allocate a slot for this person list
        if event.kind == EventKind::FollowSets {
            // get d-tag
            for tag in event.tags.iter() {
                if let Tag::Identifier { d, .. } = tag {
                    // This will allocate if missing, and will be ok if it exists
                    PersonList::allocate(d, None)?;
                }
            }
        }

        if let Some(pubkey) = GLOBALS.signer.public_key() {
            if event.pubkey == pubkey {
                // Update this data for the UI.  We don't actually process the latest event
                // until the user gives the go ahead.
                GLOBALS.people.update_latest_person_list_event_data();
            }
        }
    } else if event.kind == EventKind::RelayList {
        GLOBALS.storage.process_relay_list(event)?;
    } else if event.kind == EventKind::Repost {
        // If the content is a repost, seek the event it reposts
        for eref in event.mentions().iter() {
            match eref {
                EventReference::Id(id, optrelay, _marker) => {
                    if let Some(rurl) = optrelay {
                        let _ = GLOBALS
                            .to_overlord
                            .send(ToOverlordMessage::FetchEvent(*id, vec![rurl.to_owned()]));
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
            Some(EventReference::Id(id, _, _)) => {
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
                if let Tag::Event { id, .. } = tag {
                    GLOBALS.storage.write_relationship_by_id(
                        *id,
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
                    EventReference::Id(id, _, _) => {
                        GLOBALS.storage.write_relationship_by_id(
                            *id,
                            event.id,
                            RelationshipById::Deletion {
                                by: event.pubkey,
                                reason: reason.clone(),
                            },
                            Some(txn),
                        )?;

                        // Actually delete at this point in some cases
                        if let Some(deleted_event) = GLOBALS.storage.read_event(*id)? {
                            invalidate.push(deleted_event.id);
                            if deleted_event.pubkey != event.pubkey {
                                // No further processing if authors do not match
                                continue;
                            }
                            if !deleted_event.kind.is_feed_displayable() {
                                // Otherwise actually delete (PITA to do otherwise)
                                GLOBALS.storage.delete_event(deleted_event.id, Some(txn))?;
                            }
                        }
                    }
                    EventReference::Addr(ea) => {
                        GLOBALS.storage.write_relationship_by_addr(
                            ea.clone(),
                            event.id,
                            RelationshipByAddr::Deletion {
                                by: event.pubkey,
                                reason: reason.clone(),
                            },
                            Some(txn),
                        )?;

                        // Actually delete at this point in some cases
                        if let Some(deleted_event) = GLOBALS
                            .storage
                            .get_replaceable_event(ea.kind, ea.author, &ea.d)?
                        {
                            invalidate.push(deleted_event.id);
                            if deleted_event.pubkey != event.pubkey {
                                // No further processing if authors do not match
                                continue;
                            }
                            if !deleted_event.kind.is_feed_displayable() {
                                // Otherwise actually delete (PITA to do otherwise)
                                GLOBALS.storage.delete_event(deleted_event.id, Some(txn))?;
                            }
                        }
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
                if let Tag::Other { tag, data } = t {
                    if tag == "l" && !data.is_empty() {
                        label = &data[0];
                        if data.len() >= 2 {
                            namespace = &data[1];
                        }
                    }
                }
            }

            for tag in &event.tags {
                if let Tag::Event { id, .. } = tag {
                    GLOBALS.storage.write_relationship_by_id(
                        *id,
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
                if let Tag::Event { id, .. } = tag {
                    GLOBALS.storage.write_relationship_by_id(
                        *id,
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
                if let Tag::Event { id, .. } = tag {
                    GLOBALS.storage.write_relationship_by_id(
                        *id,
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
                if let Tag::Event { id, .. } = tag {
                    GLOBALS.storage.write_relationship_by_id(
                        *id,
                        event.id,
                        RelationshipById::ListBookmarks,
                        Some(txn),
                    )?;
                }
                if let Tag::Address {
                    kind, pubkey, d, ..
                } = tag
                {
                    if let Ok(pubkey) = PublicKey::try_from_hex_string(pubkey, true) {
                        let event_addr = EventAddr {
                            d: d.to_owned(),
                            relays: vec![],
                            kind: *kind,
                            author: pubkey,
                        };
                        GLOBALS.storage.write_relationship_by_addr(
                            event_addr,
                            event.id,
                            RelationshipByAddr::ListBookmarks,
                            Some(txn),
                        )?;
                    }
                }
            }
        }

        // BookmarkSets
        if event.kind == EventKind::BookmarkSets {
            for tag in &event.tags {
                if let Tag::Event { id, .. } = tag {
                    GLOBALS.storage.write_relationship_by_id(
                        *id,
                        event.id,
                        RelationshipById::ListBookmarks,
                        Some(txn),
                    )?;
                }
                if let Tag::Address {
                    kind, pubkey, d, ..
                } = tag
                {
                    if let Ok(pubkey) = PublicKey::try_from_hex_string(pubkey, true) {
                        let event_addr = EventAddr {
                            d: d.to_owned(),
                            relays: vec![],
                            kind: *kind,
                            author: pubkey,
                        };
                        GLOBALS.storage.write_relationship_by_addr(
                            event_addr,
                            event.id,
                            RelationshipByAddr::ListBookmarks,
                            Some(txn),
                        )?;
                    }
                }
            }
        }

        // CurationSets
        if event.kind == EventKind::CurationSets {
            for tag in &event.tags {
                if let Tag::Event { id, .. } = tag {
                    GLOBALS.storage.write_relationship_by_id(
                        *id,
                        event.id,
                        RelationshipById::Curation,
                        Some(txn),
                    )?;
                }
                if let Tag::Address {
                    kind, pubkey, d, ..
                } = tag
                {
                    if let Ok(pubkey) = PublicKey::try_from_hex_string(pubkey, true) {
                        let event_addr = EventAddr {
                            d: d.to_owned(),
                            relays: vec![],
                            kind: *kind,
                            author: pubkey,
                        };
                        GLOBALS.storage.write_relationship_by_addr(
                            event_addr,
                            event.id,
                            RelationshipByAddr::Curation,
                            Some(txn),
                        )?;
                    }
                }
            }
        }

        if event.kind == EventKind::LiveChatMessage {
            for tag in &event.tags {
                if let Tag::Address {
                    kind, pubkey, d, ..
                } = tag
                {
                    if let Ok(pubkey) = PublicKey::try_from_hex_string(pubkey, true) {
                        let event_addr = EventAddr {
                            d: d.to_owned(),
                            relays: vec![],
                            kind: *kind,
                            author: pubkey,
                        };
                        GLOBALS.storage.write_relationship_by_addr(
                            event_addr,
                            event.id,
                            RelationshipByAddr::LiveChatMessage,
                            Some(txn),
                        )?;
                    }
                }
            }
        }

        if event.kind == EventKind::BadgeAward {
            for tag in &event.tags {
                if let Tag::Address {
                    kind, pubkey, d, ..
                } = tag
                {
                    if let Ok(pubkey) = PublicKey::try_from_hex_string(pubkey, true) {
                        let event_addr = EventAddr {
                            d: d.to_owned(),
                            relays: vec![],
                            kind: *kind,
                            author: pubkey,
                        };
                        GLOBALS.storage.write_relationship_by_addr(
                            event_addr,
                            event.id,
                            RelationshipByAddr::BadgeAward,
                            Some(txn),
                        )?;
                    }
                }
            }
        }

        if event.kind == EventKind::HandlerRecommendation {
            for tag in &event.tags {
                if let Tag::Address {
                    kind, pubkey, d, ..
                } = tag
                {
                    if let Ok(pubkey) = PublicKey::try_from_hex_string(pubkey, true) {
                        let event_addr = EventAddr {
                            d: d.to_owned(),
                            relays: vec![],
                            kind: *kind,
                            author: pubkey,
                        };
                        GLOBALS.storage.write_relationship_by_addr(
                            event_addr,
                            event.id,
                            RelationshipByAddr::HandlerRecommendation,
                            Some(txn),
                        )?;
                    }
                }
            }
        }

        if event.kind == EventKind::Reporting {
            for tag in &event.tags {
                if let Tag::Event {
                    id,
                    recommended_relay_url: Some(rru),
                    ..
                } = tag
                {
                    let report = &rru.0;
                    GLOBALS.storage.write_relationship_by_id(
                        *id,
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
                if let Tag::Event { id, .. } = tag {
                    GLOBALS.storage.write_relationship_by_id(
                        *id,
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

#[allow(dead_code)]
fn update_or_allocate_person_list_from_event(
    event: &Event,
    pubkey: PublicKey,
) -> Result<(PersonList, PersonListMetadata), Error> {
    let mut txn = GLOBALS.storage.get_write_txn()?;

    // Determine PersonList and fetch Metadata
    let (list, mut metadata) = match event.kind {
        EventKind::ContactList => {
            let list = PersonList::Followed;
            let md = GLOBALS
                .storage
                .get_person_list_metadata(list)?
                .unwrap_or_default();
            (list, md)
        }
        EventKind::MuteList => {
            let list = PersonList::Muted;
            let md = GLOBALS
                .storage
                .get_person_list_metadata(list)?
                .unwrap_or_default();
            (list, md)
        }
        EventKind::FollowSets => {
            let dtag = match event.parameter() {
                Some(dtag) => dtag,
                None => return Err(ErrorKind::ListEventMissingDtag.into()),
            };
            if let Some((found_list, metadata)) = GLOBALS.storage.find_person_list_by_dtag(&dtag)? {
                (found_list, metadata)
            } else {
                // Allocate new
                let metadata = PersonListMetadata {
                    dtag,
                    title: "NEW LIST".to_owned(), // updated below
                    last_edit_time: Unixtime::now().unwrap(),
                    event_created_at: event.created_at,
                    event_public_len: 0,     // updated below
                    event_private_len: None, // updated below
                };
                let list = GLOBALS
                    .storage
                    .allocate_person_list(&metadata, Some(&mut txn))?;
                (list, metadata)
            }
        }
        _ => {
            // This function does not apply to other event kinds
            return Err(ErrorKind::NotAPersonListEvent.into());
        }
    };

    // Update metadata
    {
        metadata.event_created_at = event.created_at;

        metadata.event_public_len = event
            .tags
            .iter()
            .filter(|t| matches!(t, Tag::Pubkey { .. }))
            .count();

        if event.kind == EventKind::ContactList {
            metadata.event_private_len = None;
        } else if GLOBALS.signer.is_ready() {
            let mut private_len: Option<usize> = None;
            if let Ok(bytes) = GLOBALS.signer.decrypt_nip04(&pubkey, &event.content) {
                if let Ok(vectags) = serde_json::from_slice::<Vec<Tag>>(&bytes) {
                    private_len = Some(
                        vectags
                            .iter()
                            .filter(|t| matches!(t, Tag::Pubkey { .. }))
                            .count(),
                    );
                }
            }
            metadata.event_private_len = private_len;
        }

        if let Some(title) = event.title() {
            metadata.title = title.to_owned();
        }
    }

    // Save metadata
    GLOBALS
        .storage
        .set_person_list_metadata(list, &metadata, Some(&mut txn))?;

    txn.commit()?;

    Ok((list, metadata))
}
