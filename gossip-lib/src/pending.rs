use crate::error::Error;
use crate::globals::GLOBALS;
use crate::people::PersonList;
use nostr_types::{EventKind, RelayList, Unixtime};
use std::time::Duration;
use tokio::task;
use tokio::time::Instant;

#[derive(Debug, Clone, Hash, PartialEq)]
pub enum Pending {
    // Your relay list has changed since last advertisement, or your last advertisement
    // was over 30 days ago.
    RelayListNeverAdvertised,
    RelayListChangedSinceAdvertised,
    RelayListNotAdvertisedRecently,

    // Sync list - Your local list is out of sync with the remote list, or you haven't
    // pushed an update in 30 days.
    PersonListOutOfSync(PersonList),
    PersonListNotPublishedRecently(PersonList),
    // A posted event didn't make it to all the relays it should go to.
    // PROBLEM: Often there is a dead relay on somebody's list and so these events pile
    //          up far too much.
    // RetryPost(Id),
}

impl Pending {
    pub fn compute_pending() -> Result<Vec<Pending>, Error> {
        let mypubkey = match GLOBALS.identity.public_key() {
            Some(pk) => pk,
            None => return Ok(vec![]), // nothing pending if no identity
        };

        let now = Unixtime::now().unwrap();
        let t30days = 60 * 60 * 24 * 30;
        let t90days = 60 * 60 * 24 * 90;
        let mut pending: Vec<Pending> = Vec::new();

        let relay_lists = GLOBALS.storage.find_events(
            &[EventKind::RelayList],
            &[mypubkey],
            None,
            |_| true,
            true,
        )?;

        if relay_lists.is_empty() {
            pending.push(Pending::RelayListNeverAdvertised);
        } else {
            let stored_relay_list = GLOBALS.storage.load_relay_list()?;
            let event_relay_list = RelayList::from_event(&relay_lists[0]);

            if stored_relay_list != event_relay_list {
                pending.push(Pending::RelayListChangedSinceAdvertised);
            } else if relay_lists[0].created_at.0 + t30days < now.0 {
                pending.push(Pending::RelayListNotAdvertisedRecently);
            }
        }

        // Check each person list (if out of sync or more than 30 days ago)
        for (list, metadata) in GLOBALS.storage.get_all_person_list_metadata()?.iter() {
            // If 90 days old, should be re-synced
            if metadata.event_created_at.0 + t90days < now.0 {
                pending.push(Pending::PersonListNotPublishedRecently(*list));
                continue;
            }

            // If mismatched, should be re-synced
            let stored_hash = GLOBALS.storage.hash_person_list(*list)?;
            let last_event_hash = crate::people::hash_person_list_event(*list)?;
            if stored_hash != last_event_hash {
                pending.push(Pending::PersonListOutOfSync(*list));
                continue;
            }
        }

        Ok(pending)
    }
}

pub fn start() {
    task::spawn(async {
        let mut read_runstate = GLOBALS.read_runstate.clone();
        read_runstate.mark_unchanged();
        if !read_runstate.borrow().going_online() {
            return;
        }

        let sleep = tokio::time::sleep(Duration::from_secs(15));
        tokio::pin!(sleep);

        loop {
            tokio::select! {
                _ = &mut sleep => {
                    sleep.as_mut().reset(Instant::now() + Duration::from_secs(15));
                },
                _ = read_runstate.wait_for(|runstate| !runstate.going_online()) => break,
            }

            let pending = match Pending::compute_pending() {
                Ok(vec) => vec,
                Err(e) => {
                    tracing::error!("{:?}", e);
                    continue;
                }
            };

            *GLOBALS.pending.write() = pending;
        }

        tracing::info!("Pending checker shutdown");
    });
}
