use crate::db::{
    DbEvent, DbEventHashtag, DbEventRelationship, DbEventSeen, DbEventTag, DbPersonRelay, DbRelay,
};
use crate::error::Error;
use crate::globals::{Globals, GLOBALS};
use crate::relationship::Relationship;
use nostr_types::{Event, EventKind, Metadata, PublicKeyHex, Tag, Unixtime, Url};
use std::sync::atomic::Ordering;

// This processes a new event, saving the results into the database
// and also populating the GLOBALS maps.
pub async fn process_new_event(
    event: &Event,
    from_relay: bool,
    seen_on: Option<Url>,
    _subscription: Option<String>,
) -> Result<(), Error> {
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
        DbEvent::insert(db_event).await?;
    }

    if from_relay {
        if let Some(url) = seen_on {
            let now = Unixtime::now()?.0 as u64;

            // Save event_seen data
            let db_event_seen = DbEventSeen {
                event: event.id.as_hex_string(),
                relay: url.inner().to_owned(),
                when_seen: now,
            };
            DbEventSeen::replace(db_event_seen).await?;

            // Create the person if missing in the database
            GLOBALS
                .people
                .create_all_if_missing(&[event.pubkey.into()])
                .await?;

            // Update person_relay.last_fetched
            DbPersonRelay::upsert_last_fetched(
                event.pubkey.as_hex_string(),
                url.inner().to_owned(),
                now,
            )
            .await?;
        }
    }

    // Insert the event into globals map
    GLOBALS.events.insert(event.id, event.clone());

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
                    let url = Url::new(should_be_url);
                    if url.is_valid_relay_url() {
                        // Insert (or ignore) into relays table
                        let dbrelay = DbRelay::new(url.inner().to_owned())?;
                        DbRelay::insert(dbrelay).await?;
                    }
                }
                Tag::Pubkey {
                    pubkey,
                    recommended_relay_url: Some(should_be_url),
                    petname: _,
                } => {
                    let url = Url::new(should_be_url);
                    if url.is_valid_relay_url() {
                        // Insert (or ignore) into relays table
                        let dbrelay = DbRelay::new(url.inner().to_owned())?;
                        DbRelay::insert(dbrelay).await?;

                        // upsert person_relay.last_suggested_bytag
                        let now = Unixtime::now()?.0 as u64;
                        DbPersonRelay::upsert_last_suggested_bytag(
                            pubkey.as_hex_string(),
                            url.inner().to_owned(),
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
                    referring: id.as_hex_string(),
                    relationship: "reply".to_string(),
                    content: None,
                };
                db_event_relationship.insert().await?;
            }

            // Insert into relationships
            Globals::add_relationship(id, event.id, Relationship::Reply).await;
        }

        // reacts to
        if let Some((id, reaction, _maybe_url)) = event.reacts_to() {
            if from_relay {
                let db_event_relationship = DbEventRelationship {
                    original: event.id.as_hex_string(),
                    referring: id.as_hex_string(),
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
                        referring: id.as_hex_string(),
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
        // We only handle the user's own contact list currently
        if let Some(pubkey) = GLOBALS.signer.read().await.public_key() {
            if event.pubkey == pubkey {
                let merge: bool = GLOBALS.pull_following_merge.load(Ordering::Relaxed);
                let mut pubkeys: Vec<PublicKeyHex> = Vec::new();

                // 'p' tags represent the author's contacts
                for tag in &event.tags {
                    if let Tag::Pubkey { pubkey, .. } = tag {
                        pubkeys.push((*pubkey).into());
                        // FIXME do something with recommended_relay_url and petname
                    }
                }

                // Follow all those pubkeys, and unfollow everbody else if merge=false
                // (and the date is used to ignore if the data is outdated)
                GLOBALS
                    .people
                    .follow_all(&pubkeys, merge, event.created_at)
                    .await?;
            }
        }
    }

    // FIXME: Handle EventKind::RecommendedRelay

    // Save in event_is_new (to highlight it in the feed, if feed related)
    GLOBALS.event_is_new.write().await.push(event.id);

    Ok(())
}
