/// Relay type, aliased to the latest version
pub type Relay = crate::storage::types::Relay3;

use crate::error::{Error, ErrorKind};
use crate::person_relay::PersonRelay;
use crate::GLOBALS;
use nostr_types::{Event, Id, PublicKey, RelayUrl, RelayUsage, Unixtime};

// The functions below are all about choosing relays for some task,
// each returning `Result<Vec<RelayUrl>, Error>` (or similar)

/// This tries to generate a single RelayUrl to use for an 'e' or 'a' tag hint
pub fn recommended_relay_hint(reply_to: Id) -> Result<Option<RelayUrl>, Error> {
    let seen_on_relays: Vec<(RelayUrl, Unixtime)> =
        GLOBALS.storage.get_event_seen_on_relay(reply_to)?;

    let maybepubkey = GLOBALS.storage.read_setting_public_key();
    if let Some(pubkey) = maybepubkey {
        let my_inbox_relays: Vec<RelayUrl> = get_best_relays_min(pubkey, RelayUsage::Inbox, 0)?;

        // Find the first-best intersection
        for mir in &my_inbox_relays {
            for sor in &seen_on_relays {
                if *mir == sor.0 {
                    return Ok(Some(mir.clone()));
                }
            }
        }

        // Else fall through to seen on relays only
    }

    if let Some(sor) = seen_on_relays.first() {
        return Ok(Some(sor.0.clone()));
    }

    Ok(None)
}

// Which relays are best for a reply to this event (used to find replies to this event)
pub fn relays_for_reply(event: &Event) -> Result<Vec<RelayUrl>, Error> {
    let mut seen_on: Vec<RelayUrl> = GLOBALS
        .storage
        .get_event_seen_on_relay(event.id)?
        .drain(..)
        .map(|(url, _time)| url)
        .collect();

    let inbox: Vec<RelayUrl> = get_best_relays_fixed(event.pubkey, RelayUsage::Inbox)?;

    // Take all inbox relays, and up to 2 seen_on relays that aren't inbox relays
    let mut answer = inbox;
    let mut extra = 2;
    for url in seen_on.drain(..) {
        if extra == 0 {
            break;
        }
        if answer.contains(&url) {
            continue;
        }
        answer.push(url);
        extra -= 1;
    }

    Ok(answer)
}

// Which relays should an event be posted to (that it hasn't already been
// seen on)?
pub fn relays_for_event(event: &Event) -> Result<Vec<RelayUrl>, Error> {
    let mut relay_urls: Vec<RelayUrl> = Vec::new();

    // Get all of the relays that we write to
    let write_relay_urls: Vec<RelayUrl> = Relay::choose_relay_urls(Relay::WRITE, |_| true)?;
    relay_urls.extend(write_relay_urls);

    // Get 'read' relays for everybody tagged in the event.
    let mut tagged_pubkeys: Vec<PublicKey> = event
        .tags
        .iter()
        .filter_map(|t| {
            if let Ok((pubkey, _, _)) = t.parse_pubkey() {
                Some(pubkey)
            } else {
                None
            }
        })
        .collect();
    for pubkey in tagged_pubkeys.drain(..) {
        let best_relays: Vec<RelayUrl> = get_best_relays_fixed(pubkey, RelayUsage::Inbox)?;
        relay_urls.extend(best_relays);
    }

    // Remove all the 'seen_on' relays for this event
    let seen_on: Vec<RelayUrl> = GLOBALS
        .storage
        .get_event_seen_on_relay(event.id)?
        .iter()
        .map(|(url, _time)| url.to_owned())
        .collect();
    relay_urls.retain(|r| !seen_on.contains(r));

    relay_urls.sort();
    relay_urls.dedup();

    Ok(relay_urls)
}

/// Get best relays for a person
///
/// This is for the given a direction (read or write).
/// This does not handle DM usage, use get_dm_relays() for that.
///
/// Take the best `num_relays_per_person` relays from their declared
/// relays (skipping relays that are banned or with rank=0)
/// and we come up short, use the best alternatives.
pub fn get_best_relays_fixed(pubkey: PublicKey, usage: RelayUsage) -> Result<Vec<RelayUrl>, Error> {
    let num = GLOBALS.storage.read_setting_num_relays_per_person() as usize;
    Ok(get_best_relays_with_score(pubkey, usage, num)?
        .drain(..)
        .take(num)
        .map(|(url, _score)| url)
        .collect())
}

/// Get best relays for a person
///
/// This is for the given a direction (read or write).
/// This does not handle DM usage, use get_dm_relays() for that.
///
/// take all relays from their declared relays (skipping relays that are
/// banned or with rank=0) and if we come up short of `min`, use the
/// best alternatives.
pub fn get_best_relays_min(
    pubkey: PublicKey,
    usage: RelayUsage,
    min: usize,
) -> Result<Vec<RelayUrl>, Error> {
    Ok(get_best_relays_with_score(pubkey, usage, min)?
        .drain(..)
        .map(|(url, _score)| url)
        .collect())
}

/// Get the best relays for a person, given a direction (read or write).
/// This does not handle DM usage, use get_dm_relays() for that.
///
/// This takes ALL of their relay-list declared relays (except anything we banned
/// with rank=0), and if that is less than `min` it includes the best additional
/// relays it can to make up `min` relays.
pub fn get_best_relays_with_score(
    pubkey: PublicKey,
    usage: RelayUsage,
    min: usize,
) -> Result<Vec<(RelayUrl, u64)>, Error> {
    if usage != RelayUsage::Outbox && usage != RelayUsage::Inbox {
        return Err((ErrorKind::UnsupportedRelayUsage, file!(), line!()).into());
    }

    let now = Unixtime::now();

    // Load person relays, filtering out banned URLs
    let mut person_relays: Vec<PersonRelay> = GLOBALS
        .storage
        .get_person_relays(pubkey)?
        .drain(..)
        .filter(|pr| !crate::storage::Storage::url_is_banned(&pr.url))
        .collect();

    // Load associated relay records, and compute scores
    let mut candidates: Vec<(RelayUrl, u64)> = Vec::new();
    for pr in person_relays.drain(..) {
        // Compute how strongly it associates to them
        let association_rank = pr.association_rank(now, usage == RelayUsage::Outbox);

        // Load the relay so we can get more score-determining data
        let relay = GLOBALS.storage.read_or_create_relay(&pr.url, None)?;

        if relay.rank == 0 {
            continue;
        }
        if relay.should_avoid() {
            continue;
        }

        let mut score = if association_rank >= 20 {
            // Do not modulate scores of declared relays.
            20
        } else {
            // Compute a score based on the association_rank and also
            // whether or not the relay is any good
            (association_rank as f32
                * (relay.rank as f32 / 3.0)
                * (0.75 + 0.25 * relay.success_rate())) as u64
        };

        // Cap scores at 20
        if score > 20 {
            score = 20;
        }

        candidates.push((pr.url, score));
    }

    // Sort
    candidates.sort_by(|(_, score1), (_, score2)| score2.cmp(score1));

    // Take all score=20 (declared or very preferred), or while we haven't reached min
    let mut relays: Vec<(RelayUrl, u64)> = candidates
        .drain(..)
        .enumerate()
        .take_while(|(i, (_u, s))| *s >= 20 || *i <= min)
        .map(|(_i, (u, s))| (u, s))
        .collect();

    // If we still haven't got minimum relays, use our own relays
    if relays.len() < min {
        let how_many_more = min - relays.len();
        if usage == RelayUsage::Outbox {
            // substitute our read relays
            let additional: Vec<(RelayUrl, u64)> = GLOBALS
                .storage
                .filter_relays(|r| {
                    // not already in their list
                    !relays.iter().any(|(url, _)| *url == r.url) && r.has_usage_bits(Relay::READ)
                })?
                .iter()
                .map(|r| (r.url.clone(), 1))
                .take(how_many_more)
                .collect();
            relays.extend(additional);
        } else {
            // substitute our write relays
            let additional: Vec<(RelayUrl, u64)> = GLOBALS
                .storage
                .filter_relays(|r| {
                    // not already in their list
                    !relays.iter().any(|(url, _)| *url == r.url) && r.has_usage_bits(Relay::WRITE)
                })?
                .iter()
                .map(|r| (r.url.clone(), 1))
                .take(how_many_more)
                .collect();
            relays.extend(additional);
        }
    }

    Ok(relays)
}

/// This gets NIP-17 DM relays only.
///
/// At the time of writing, not many people have these specified, in which case
/// the caller should fallback to write relays and NIP-04.
pub fn get_dm_relays(pubkey: PublicKey) -> Result<Vec<RelayUrl>, Error> {
    let mut output: Vec<RelayUrl> = Vec::new();
    for pr in GLOBALS.storage.get_person_relays(pubkey)?.drain(..) {
        let relay = GLOBALS.storage.read_or_create_relay(&pr.url, None)?;
        if relay.rank == 0 {
            continue;
        }
        if relay.should_avoid() {
            continue;
        }

        if pr.dm {
            output.push(pr.url)
        }
    }
    Ok(output)
}

pub fn get_others_relays(
    recipients: &[PublicKey],
    usage: RelayUsage,
) -> Result<Vec<RelayUrl>, Error> {
    let mut relay_urls: Vec<RelayUrl> = Vec::new();
    for pubkey in recipients {
        let best_relays: Vec<RelayUrl> = get_best_relays_fixed(*pubkey, usage)?;
        relay_urls.extend(best_relays);
    }
    Ok(relay_urls)
}
