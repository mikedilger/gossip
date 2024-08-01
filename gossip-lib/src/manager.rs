use crate::comms::{RelayJob, ToMinionMessage};
use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::overlord::Minion;
use crate::pending::PendingItem;
use crate::relay::Relay;
use dashmap::mapref::entry::Entry;
use nostr_types::RelayUrl;

/// This is the main entry point for running a set of jobs on a set of relays.
/// You specify the relays you prefer, in order of preferences, and the number
/// of relays you want to engage.
///
/// If a relay engagement fails it is skipped and the next one tried until count
/// is achieved.
///
/// This function returns quickly, as it spawns a separate task to do the engagement
/// so you won't get any feedback.
pub(crate) async fn run_job_on_relays(urls: Vec<RelayUrl>, jobs: Vec<RelayJob>, count: usize) {
    // Keep engaging relays until `count` engagements were successful
    // Do from a spawned task so that we don't hold up the overlord
    let _join_handle = tokio::spawn(async move {
        let mut successes: usize = 0;
        for url in urls.iter() {
            if let Ok(_) = engage_minion(url.to_owned(), jobs.clone()).await {
                successes += 1;
                if successes >= count {
                    break;
                }
            }
        }
    });
}

pub(crate) async fn engage_minion(url: RelayUrl, jobs: Vec<RelayJob>) -> Result<(), Error> {
    let relay = GLOBALS.storage.read_or_create_relay(&url, None)?;

    if GLOBALS
        .storage
        .read_setting_relay_connection_requires_approval()
    {
        match relay.allow_connect {
            Some(true) => (), // fall through
            Some(false) => return Err(ErrorKind::EngageDisallowed.into()),
            None => {
                // Save the engage_minion request and Ask the user
                GLOBALS.pending.insert(PendingItem::RelayConnectionRequest {
                    relay: url.clone(),
                    jobs: jobs.clone(),
                });
                return Err(ErrorKind::EngagePending.into());
            }
        }
    } // else fall through

    engage_minion_inner(relay, url, jobs).await
}

pub(crate) async fn engage_minion_inner(
    relay: Relay,
    url: RelayUrl,
    mut jobs: Vec<RelayJob>,
) -> Result<(), Error> {
    // Do not connect if we are offline
    if GLOBALS.storage.read_setting_offline() {
        return Err(ErrorKind::Offline.into());
    }

    if jobs.is_empty() {
        return Err(ErrorKind::EmptyJob.into());
    }

    // don't connect while avoiding this relay
    if relay.should_avoid() {
        return Err(ErrorKind::EngageDisallowed.into());
    }

    let entry = GLOBALS.connected_relays.entry(url.clone());

    if let Entry::Occupied(mut oe) = entry {
        // We are already connected. Send it the jobs
        for job in jobs.drain(..) {
            let _ = GLOBALS.to_minions.send(ToMinionMessage {
                target: url.as_str().to_owned(),
                payload: job.payload.clone(),
            });

            let vec = oe.get_mut();

            // Record the job:
            // If the relay already has a job of the same RelayConnectionReason
            // and that reason is not persistent, then this job replaces that
            // one (e.g. FetchAugments)
            if !job.reason.persistent() {
                if let Some(pos) = vec.iter().position(|e| e.reason == job.reason) {
                    vec[pos] = job;
                    return Ok(());
                }
            }
            vec.push(job);
        }
    } else {
        // Start up the minion
        let mut minion = Minion::new(url.clone()).await?;
        let payloads = jobs.iter().map(|job| job.payload.clone()).collect();
        let abort_handle = GLOBALS
            .minions
            .write()
            .spawn(async move { minion.handle(payloads).await });
        let id = abort_handle.id();
        GLOBALS.minions_task_url.insert(id, url.clone());

        entry.insert(jobs);
    }

    Ok(())
}
