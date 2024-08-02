// These options should cover all the cases of needing to choose a set of relays:
//
// Relay::choose_relay_urls(Relay::READ, |_| true)?;  // all ours
// Relay::choose_relay_urls(Relay::WRITE, |_| true)?; // all ours
// Relay::choose_relay_urls(Relay::DM, |_| true)?;    // all ours
// relay::get_some_pubkey_outboxes(pubkey)?  // for subscribing to theirs
// relay::get_all_pubkey_outboxes(pubkey)?   // informational
// relay::get_all_pubkey_inboxes(pubkey)?    // for replying to them
// relay::get_dm_relays(pubkey)?             // for DMs to them
// relay::get_best_relays_with_score(pubkey, usage, score_factors) // for relay picker, and internal
// relay::recommended_relay_hint(reply_to_id)?    // for a hint
// relay::relays_for_seeking_replies(&event)?     // to find replies
// relay::relays_to_post_to(&event)?              // where to post
// future: get_all_pubkey_outboxes_for_batch_search(pubkey)?     // for seeker exhaustive search

/// Relay type, aliased to the latest version
pub type Relay = crate::storage::types::Relay3;

use crate::error::{Error, ErrorKind};
use crate::person_relay::PersonRelay;
use crate::GLOBALS;
use nostr_types::{Event, EventKind, Id, PublicKey, RelayUrl, RelayUsage, Unixtime};

// Get `num_relays_per_prson` outboxes to subscribe to their events
pub fn get_some_pubkey_outboxes(pubkey: PublicKey) -> Result<Vec<RelayUrl>, Error> {
    let num = GLOBALS.storage.read_setting_num_relays_per_person() as usize;
    let relays = get_best_relays_with_score(
        pubkey,
        RelayUsage::Outbox,
        ScoreFactors::RelayScorePlusConnected,
    )?
    .iter()
    .take(num)
    .map(|(url, _score)| url.to_owned())
    .collect();
    Ok(relays)
}

// Get all person outboxes for informational
pub fn get_all_pubkey_outboxes(pubkey: PublicKey) -> Result<Vec<RelayUrl>, Error> {
    let relays = get_best_relays_with_score(
        pubkey,
        RelayUsage::Outbox,
        ScoreFactors::RelayScorePlusConnected,
    )?
    .iter()
    .map(|(url, _score)| url.to_owned())
    .collect();
    Ok(relays)
}

// Get all the inboxes to post something to them
// (also if they have none, we substitute our write relays)
pub fn get_all_pubkey_inboxes(pubkey: PublicKey) -> Result<Vec<RelayUrl>, Error> {
    // Why 0.125?
    //   if declared they will get an association score of at least 1.0
    //   by default based on relay rank, they will get a relay score of 0.33333
    //   modified by success rate, and 50% success rate will give 75% of this number, which is 0.25
    //   plus-connected cuts it in half if not connected, so 0.125, and we want to include all
    //   declared relays even that aren't connected down to 50% success rate.
    let mut relays: Vec<(RelayUrl, f32)> =
        get_best_relays_with_score(pubkey, RelayUsage::Inbox, ScoreFactors::RelayScore)?
            .drain(..)
            .filter(|(_, score)| *score > 0.125)
            .collect();

    let num = GLOBALS.storage.read_setting_num_relays_per_person() as usize;
    let how_many_more = num - relays.len();
    if how_many_more > 0 {
        // substitute our write relays
        let additional: Vec<(RelayUrl, f32)> = GLOBALS
            .storage
            .filter_relays(|r| {
                // not already in their list
                !relays.iter().any(|(url, _)| *url == r.url) && r.has_usage_bits(Relay::WRITE)
            })?
            .iter()
            .map(|r| (r.url.clone(), 0.01))
            .take(how_many_more)
            .collect();
        relays.extend(additional);
    }

    Ok(relays.drain(..).map(|(url, _score)| url).collect())
}

/// This gets NIP-17 DM relays only.
///
/// At the time of writing, not many people have these specified, in which case
/// the caller should fallback to write relays and NIP-04.
pub fn get_dm_relays(pubkey: PublicKey) -> Result<Vec<RelayUrl>, Error> {
    let mut output: Vec<RelayUrl> = Vec::new();
    for pr in GLOBALS.storage.get_person_relays(pubkey)?.drain(..) {
        let relay = GLOBALS.storage.read_or_create_relay(&pr.url, None)?;

        if relay.should_avoid() {
            continue;
        }

        if pr.dm {
            output.push(pr.url)
        }
    }
    Ok(output)
}

// The functions below are all about choosing relays for some task,
// each returning `Result<Vec<RelayUrl>, Error>` (or similar)

/// This tries to generate a single RelayUrl to use for an 'e' or 'a' tag hint
pub fn recommended_relay_hint(reply_to: Id) -> Result<Option<RelayUrl>, Error> {
    let seen_on_relays: Vec<(RelayUrl, Unixtime)> =
        GLOBALS.storage.get_event_seen_on_relay(reply_to)?;

    let maybepubkey = GLOBALS.storage.read_setting_public_key();
    if let Some(pubkey) = maybepubkey {
        let my_inbox_relays: Vec<RelayUrl> = get_all_pubkey_inboxes(pubkey)?;

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
// FIXME this may go away once seeker uses 'sort relays' below, I'm not sure.
pub fn relays_for_seeking_replies(event: &Event) -> Result<Vec<RelayUrl>, Error> {
    let mut relays: Vec<RelayUrl> = Vec::new();

    // Inboxes of the author
    relays.extend(get_all_pubkey_inboxes(event.pubkey)?);

    // Inboxes of the 'p' tagged people, up to num
    //for (tagged_pubkey, _opt_relay_url, _opt_marker) in event.people() {
    //  relays.extend(get_all_reasonable_boxes(tagged_pubkey, RelayUsage::Inbox, num)?);
    //}

    // Seen on relays
    let mut seen_on: Vec<RelayUrl> = GLOBALS
        .storage
        .get_event_seen_on_relay(event.id)?
        .drain(..)
        .map(|(url, _time)| url)
        .collect();

    // Take all inbox relays, and up to 2 seen_on relays that aren't inbox relays
    let mut extra = 2;
    for url in seen_on.drain(..) {
        if extra == 0 {
            break;
        }
        if relays.contains(&url) {
            continue;
        }
        relays.push(url);
        extra -= 1;
    }

    Ok(relays)
}

// Which relays should an event be posted to (that it hasn't already been
// seen on)?  DO NOT USE for NIP-17 (we can't tell the recipient)
pub fn relays_to_post_to(event: &Event) -> Result<Vec<RelayUrl>, Error> {
    let mut relays: Vec<RelayUrl> = Vec::new();

    if event.kind == EventKind::GiftWrap || event.kind == EventKind::DmChat {
        return Err(ErrorKind::Internal(
            "DO NOT USE relays_to_post_to() for Giftwrap DMs".to_string(),
        )
        .into());
    }

    // All of my outboxes
    relays.extend(Relay::choose_relay_urls(Relay::WRITE, |_| true)?);

    // Inbox (or DM) relays of tagged people
    let mut tagged_pubkeys: Vec<PublicKey> = event.people().iter().map(|(pk, _, _)| *pk).collect();
    for pubkey in tagged_pubkeys.drain(..) {
        let user_relays = get_all_pubkey_inboxes(pubkey)?;
        if event.kind == EventKind::EncryptedDirectMessage {
            let dm_relays = get_dm_relays(pubkey)?;
            if dm_relays.is_empty() {
                relays.extend(user_relays);
            } else {
                relays.extend(dm_relays);
            }
        } else {
            relays.extend(user_relays);
        }
    }

    // Remove all the 'seen_on' relays for this event
    let seen_on: Vec<RelayUrl> = GLOBALS
        .storage
        .get_event_seen_on_relay(event.id)?
        .iter()
        .map(|(url, _time)| url.to_owned())
        .collect();
    relays.retain(|r| !seen_on.contains(r));

    relays.sort();
    relays.dedup();

    Ok(relays)
}

pub enum ScoreFactors {
    None,
    RelayScore,
    RelayScorePlusConnected,
}

/// Only RelayUsage::Outbox and RelayUsage::Inbox are supported.
///
/// Output scores range from 0.0 to 1.0
pub fn get_best_relays_with_score(
    pubkey: PublicKey,
    usage: RelayUsage,
    score_factors: ScoreFactors,
) -> Result<Vec<(RelayUrl, f32)>, Error> {
    if usage != RelayUsage::Outbox && usage != RelayUsage::Inbox {
        return Err((ErrorKind::UnsupportedRelayUsage, file!(), line!()).into());
    }

    // Load person relays, filtering out banned URLs
    let mut person_relays: Vec<PersonRelay> = GLOBALS
        .storage
        .get_person_relays(pubkey)?
        .drain(..)
        .filter(|pr| !crate::storage::Storage::url_is_banned(&pr.url))
        .collect();

    let mut output: Vec<(RelayUrl, f32)> = Vec::new();

    let now = Unixtime::now();
    for pr in person_relays.drain(..) {
        // Get their association to that relay
        let association_score = pr.association_score(now, usage);

        let relay = GLOBALS.storage.read_or_create_relay(&pr.url, None)?;
        if relay.should_avoid() {
            continue;
        }

        let multiplier = match score_factors {
            ScoreFactors::None => 1.0,
            ScoreFactors::RelayScore => relay.score(),
            ScoreFactors::RelayScorePlusConnected => relay.score_plus_connected(),
        };

        // Multiply them (max is the relay score, or the association score)
        let score = association_score * multiplier;

        output.push((pr.url, score));
    }

    output.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    Ok(output)
}

/*
/// For seeking a KNOWN event (e.g. thread climbing, quoting) that we do not have, but which
/// may be related to certain relays or certain people, this function determines which relays
/// to look on in priority order.  The list is not pruned or limited so that the seeker can
/// keep looking and never give up until it either finds it or it tried all the places
/// that we thought might be viable.
pub fn sort_relays(
    mut hinted: Vec<RelayUrl>,
    mut related_seen_on: Vec<RelayUrl>,
    mut inboxes: Vec<PublicKey>,
    mut outboxes: Vec<PublicKey>,
) -> Result<Vec<RelayUrl>, Error> {
    struct RelayData {
        score: f32,
        bonus: f32, // fractions of 10
    }

    // For each URL, keep the relay record and a score
    let mut map: HashMap<RelayUrl, RelayData> = HashMap::new();

    // Load each hinted relay, gets 1 bonus point
    for url in hinted.drain(..) {
        let relay = GLOBALS.storage.read_or_create_relay(&url, None)?;
        let score = relay.score_plus_connected();
        let relay_data = RelayData { score, bonus: 1.0 };
        map.insert(url, relay_data);
    }

    // Load each seen on relay, gets 2 bonus points
    for url in related_seen_on.drain(..) {
        if let Some(data) = map.get_mut(&url) {
            data.bonus += 2.0;
        } else {
            let relay = GLOBALS.storage.read_or_create_relay(&url, None)?;
            let score = relay.score_plus_connected();
            let relay_data = RelayData { score, bonus: 2.0 };
            map.insert(url, relay_data);
        }
    }

    // Load inboxes
    for pk in inboxes.drain(..) {
        let mut relays: Vec<(RelayUrl, f32)> =
            get_best_relays_with_score(pk, RelayUsage::Inbox, ScoreFactors::None)?;

        for (url, pscore) in relays.drain(..) {
            if let Some(data) = map.get_mut(&url) {
                data.bonus += 5.0 * pscore;
            } else {
                let relay = GLOBALS.storage.read_or_create_relay(&url, None)?;
                let score = relay.score_plus_connected();
                let relay_data = RelayData {
                    score,
                    bonus: 5.0 * pscore,
                };
                map.insert(url, relay_data);
            }
        }
    }

    // Load outboxes
    for pk in outboxes.drain(..) {
        let mut relays: Vec<(RelayUrl, f32)> =
            get_best_relays_with_score(pk, RelayUsage::Outbox, ScoreFactors::None)?;

        for (url, pscore) in relays.drain(..) {
            if let Some(data) = map.get_mut(&url) {
                data.bonus += 5.0 * pscore;
            } else {
                let relay = GLOBALS.storage.read_or_create_relay(&url, None)?;
                let score = relay.score_plus_connected();
                let relay_data = RelayData {
                    score,
                    bonus: 5.0 * pscore,
                };
                map.insert(url, relay_data);
            }
        }
    }

    let mut vec: Vec<(RelayUrl, f32)> = map
        .iter()
        .map(|(url, data)| (url.to_owned(), data.score * 0.1 * data.bonus))
        .collect();
    vec.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    Ok(vec.iter().map(|(url, _)| url.to_owned()).collect())
}
*/
