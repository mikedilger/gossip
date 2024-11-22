use crate::comms::ToOverlordMessage;
use crate::error::Error;
use crate::globals::GLOBALS;
use crate::people::{PersonList, PersonListMetadata};
use crate::storage::table::Table;
use nostr_types::{Event, RelayUrl};

// EventKind::Metadata
pub fn process_metadata(event: &Event) -> Result<(), Error> {
    use nostr_types::Metadata;

    let metadata: Metadata = serde_json::from_str(&event.content)?;
    GLOBALS
        .people
        .update_metadata(&event.pubkey, metadata, event.created_at)?;
    Ok(())
}

// EventKind::HandlerRecommendation
// Collect handler recommendations, then fetch the handler information
pub fn process_handler_recommendation(event: &Event) -> Result<(), Error> {
    use crate::storage::types::HandlerKey;
    use nostr_types::{EventKind, NAddr};

    // NOTE: We don't care what 'd' kind is given, we collect these for all kinds.

    let mut naddrs: Vec<NAddr> = Vec::new();
    let mut d = "".to_owned();

    for tag in &event.tags {
        if tag.get_index(0) == "d" {
            d = tag.get_index(1).to_owned();
        }

        let (naddr, marker) = match tag.parse_address() {
            Ok(pair) => pair,
            Err(_) => continue,
        };
        let marker = match marker {
            Some(m) => m,
            None => continue,
        };
        if marker != "web" {
            continue;
        }

        if naddr.kind != EventKind::HandlerInformation {
            continue;
        };

        // We need a relay to load the handler from
        if naddr.relays.is_empty() {
            continue;
        }

        naddrs.push(naddr);
    }

    if naddrs.is_empty() {
        return Ok(());
    }

    // If it is ours (e.g. from another client), update our local recommendation bits
    if let Some(pk) = GLOBALS.identity.public_key() {
        if event.pubkey == pk {
            if let Ok(kindnum) = d.parse::<u32>() {
                let kind: EventKind = kindnum.into();
                let configured_handlers: Vec<(HandlerKey, bool, bool)> =
                    GLOBALS.db().read_configured_handlers(kind)?;
                for (key, enabled, recommended) in configured_handlers.iter() {
                    let event_recommended =
                        naddrs.iter().any(|naddr| *naddr == key.as_naddr(vec![]));
                    if event_recommended != *recommended {
                        GLOBALS.db().write_configured_handler(
                            kind,
                            key.clone(),
                            *enabled,
                            event_recommended,
                            None,
                        )?;
                    }
                }
            }
        }
    }

    for naddr in naddrs {
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::FetchNAddr(naddr));
    }

    Ok(())
}

// EventKind::HandlerInformation
pub fn process_handler_information(event: &Event) -> Result<(), Error> {
    use crate::storage::types::Handler;
    use crate::storage::HandlersTable;

    // If event kind handler information, add to database
    if let Some(mut handler) = Handler::from_31990(event) {
        HandlersTable::write_record(&mut handler, None)?;

        // Also add entry to configured_handlers for each kind
        for kind in handler.kinds {
            // If we already have this handler, do not clobber the
            // user's 'enabled' flag
            let existing = GLOBALS.db().read_configured_handlers(kind)?;
            if existing.iter().any(|(hk, _, _)| *hk == handler.key) {
                continue;
            }

            // Write configured handler, enabled by default
            GLOBALS.db().write_configured_handler(
                kind,
                handler.key.clone(),
                true,  // enabled
                false, // recommended
                None,
            )?;
        }
    }

    Ok(())
}

// EventKind::ContactList
pub fn process_contact_list(event: &Event) -> Result<(), Error> {
    if let Some(pubkey) = GLOBALS.identity.public_key() {
        if event.pubkey == pubkey {
            // Updates stamps and counts, does NOT change membership
            let (_personlist, _metadata) = update_or_allocate_person_list_from_event(event)?;
        } else {
            process_somebody_elses_contact_list(event, false)?;
        }
    } else {
        process_somebody_elses_contact_list(event, false)?;
    }

    Ok(())
}

// EventKind::MuteList
pub fn process_mute_list(event: &Event, ours: bool) -> Result<(), Error> {
    if ours {
        let (_personlist, _metadata) = update_or_allocate_person_list_from_event(event)?;
    }

    Ok(())
}

// EventKind::FollowSets
pub fn process_follow_sets(event: &Event, ours: bool) -> Result<(), Error> {
    if ours {
        let (_personlist, _metadata) = update_or_allocate_person_list_from_event(event)?;
    }

    Ok(())
}

// EventKind::RelayList
pub fn process_relay_list(event: &Event) -> Result<(), Error> {
    GLOBALS.db().process_relay_list(event, false, None)?;

    // Let the seeker know we now have relays for this author, in case the seeker
    // wants to update it's state
    // (we might not, but by this point we have tried)
    GLOBALS.seeker.found_author_relays(event.pubkey);

    // the following also refreshes scores before it picks relays
    let _ = GLOBALS
        .to_overlord
        .send(ToOverlordMessage::RefreshScoresAndPickRelays);

    Ok(())
}

// EventKind::DmRelayList
pub fn process_dm_relay_list(event: &Event) -> Result<(), Error> {
    GLOBALS.db().process_dm_relay_list(event, None)?;

    Ok(())
}

// EventKind::Repost
pub fn process_repost(event: &Event, verify: bool) -> Result<(), Error> {
    use crate::misc::Freshness;
    use crate::people::People;
    use nostr_types::EventReference;

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
        crate::process::process_new_event(&inner_event, None, None, verify, false)?;

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

    Ok(())
}

// EventKind::NostrConnect
pub fn process_nostr_connect(event: &Event, seen_on: Option<RelayUrl>) -> Result<(), Error> {
    crate::nostr_connect_server::handle_command(event, seen_on)?;

    Ok(())
}

// EventKind::UserServerList
pub fn process_user_server_list(event: &Event, ours: bool) -> Result<(), Error> {
    if ours {
        // Update blossom servers
        let mut servers: String = "".to_owned();
        let mut virgin: bool = true;
        for tag in &event.tags {
            if tag.tagname() == "server" {
                if !virgin {
                    servers.push('\n');
                }
                servers.push_str(tag.value());
                virgin = false;
            }
        }
        GLOBALS.db().write_setting_blossom_servers(&servers, None)?;
    }

    Ok(())
}

pub fn process_somebody_elses_contact_list(event: &Event, force: bool) -> Result<(), Error> {
    use crate::people::PersonList;
    use crate::storage::Storage;
    use nostr_types::{RelayList, RelayListUsage, SimpleRelayList};

    // Only if we follow them... update their followings record and the FoF
    if GLOBALS
        .people
        .is_person_in_list(&event.pubkey, PersonList::Followed)
    {
        Storage::update_followings_and_fof_from_contact_list(event, None)?;
    }

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
            .db()
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

// This updates the event data and maybe the title, but it does NOT update the list
// (that happens only when the user overwrites/merges)
fn update_or_allocate_person_list_from_event(
    event: &Event,
) -> Result<(PersonList, PersonListMetadata), Error> {
    use nostr_types::{EventKind, Tag};

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
            if let Ok(bytes) = GLOBALS.identity.decrypt(&event.pubkey, &event.content) {
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
        .db()
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
