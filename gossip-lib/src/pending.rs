use crate::comms::RelayJob;
use crate::error::Error;
use crate::globals::GLOBALS;
use crate::nip46::ParsedCommand;
use crate::people::PersonList;
use nostr_types::{EventKind, PublicKey, RelayList, RelayUrl, Unixtime};
use parking_lot::RwLock as PRwLock;
use parking_lot::RwLockReadGuard as PRwLockReadGuard;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use tokio::task;
use tokio::time::Instant;

#[derive(Debug, Clone, Hash, PartialEq)]
pub enum PendingItem {
    /// Relay picker wants to connect to this relay
    RelayConnectionRequest {
        relay: RelayUrl,
        jobs: Vec<RelayJob>,
    },

    /// Relay picker wants to authenticate to this relay with a private key signature
    RelayAuthenticationRequest {
        account: PublicKey,
        relay: RelayUrl,
    },

    /// A NIP46 remote signing request was received and requires permission
    Nip46Request {
        client_name: String,
        account: PublicKey,
        command: crate::nip46::ParsedCommand,
    },

    // Your relay list has changed since last advertisement, or your last advertisement
    // was over 30 days ago.
    RelayListNeverAdvertised,
    RelayListChangedSinceAdvertised,
    RelayListNotAdvertisedRecently,

    // Sync list - Your local list is out of sync with the remote list, or you haven't
    // pushed an update in 30 days.
    PersonListNeverPublished(PersonList),
    PersonListOutOfSync(PersonList),
    PersonListNotPublishedRecently(PersonList),
    // A posted event didn't make it to all the relays it should go to.
    // PROBLEM: Often there is a dead relay on somebody's list and so these events pile
    //          up far too much.
    // RetryPost(Id),
}

pub struct Pending {
    /// Pending actions
    pending: PRwLock<Vec<(PendingItem, u64)>>,

    /// Current hash of the pending map
    pending_hash: PRwLock<u64>,
}

impl Default for Pending {
    fn default() -> Pending {
        Self::new()
    }
}

fn calculate_pending_hash(vec: &Vec<(PendingItem, u64)>) -> u64 {
    let mut s = DefaultHasher::new();
    vec.hash(&mut s);
    s.finish()
}
impl PendingItem {
    fn matches(&self, other: &PendingItem) -> bool {
        match self {
            PendingItem::RelayConnectionRequest { relay: a_url, .. } => match other {
                PendingItem::RelayConnectionRequest { relay: b_url, .. } => a_url == b_url,
                _ => false,
            },
            item => item == other,
        }
    }
}

impl Pending {
    pub fn new() -> Self {
        let pending = PRwLock::new(Vec::new());
        let pending_hash = PRwLock::new(calculate_pending_hash(&pending.read()));
        Self {
            pending,
            pending_hash,
        }
    }

    /// returns the current hash of the pending list
    pub fn hash(&self) -> u64 {
        let _list = self.pending.read();
        *self.pending_hash.read()
    }

    pub fn read(&self) -> PRwLockReadGuard<Vec<(PendingItem, u64)>> {
        self.pending.read()
    }

    /// Insert a pending item
    /// will only insert each pending item once attempting to merge requests
    /// timestamp will be of first entry into list
    /// pending_hash will be updated after sorting
    pub fn insert(&self, item: PendingItem) -> bool {
        let mut existing = false;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.pending.write().iter_mut().for_each(|(entry, _)| {
            if entry.matches(&item) {
                match entry {
                    // merge jobs for connection requests to the same relay
                    PendingItem::RelayConnectionRequest { jobs, .. } => {
                        let new_jobs = match &item {
                            PendingItem::RelayConnectionRequest { jobs, .. } => Some(jobs),
                            _ => None,
                        };
                        if let Some(new_jobs) = new_jobs {
                            jobs.extend(new_jobs.iter().cloned());
                        }
                        existing = true;
                    }
                    _ => {
                        existing = true;
                    }
                }
            }
        });

        if !existing {
            self.pending.write().push((item, now));
            {
                let mut list = self.pending.write();
                list.sort_by(|a, b| b.1.cmp(&a.1));
                *self.pending_hash.write() = calculate_pending_hash(&list);
            }
            true
        } else {
            false
        }
    }

    pub fn take_relay_connection_request(
        &self,
        relay_url: &RelayUrl,
    ) -> Option<(RelayUrl, Vec<RelayJob>)> {
        let mut pending = self.pending.write();
        let index = pending.iter().position(|(item, _)| matches!(item, PendingItem::RelayConnectionRequest { relay, .. } if relay == relay_url));
        if let Some(index) = index {
            let entry = pending.remove(index);
            *self.pending_hash.write() = calculate_pending_hash(&pending);
            match entry.0 {
                PendingItem::RelayConnectionRequest { relay, jobs } => Some((relay, jobs)),
                _ => None,
            }
        } else {
            None
        }
    }

    pub fn take_relay_authentication_request(
        &self,
        account: &PublicKey,
        relay_url: &RelayUrl,
    ) -> Option<(PublicKey, RelayUrl)> {
        let mut pending = self.pending.write();
        let index = pending.iter().position(|(item, _)| matches!(item, PendingItem::RelayAuthenticationRequest { account: pubkey, relay } if relay == relay_url && pubkey == account));
        if let Some(index) = index {
            let entry = pending.remove(index);
            *self.pending_hash.write() = calculate_pending_hash(&pending);
            match entry.0 {
                PendingItem::RelayAuthenticationRequest { account, relay } => {
                    Some((account, relay))
                }
                _ => None,
            }
        } else {
            None
        }
    }

    pub fn take_nip46_request(
        &self,
        account: &PublicKey,
        command: &ParsedCommand,
    ) -> Option<(String, PublicKey, ParsedCommand)> {
        let mut pending = self.pending.write();
        let index = pending.iter().position(|(item, _)| matches!(item, PendingItem::Nip46Request { account: item_account, command: item_command, .. } if item_account == account && item_command == command));
        if let Some(index) = index {
            let entry = pending.remove(index);
            *self.pending_hash.write() = calculate_pending_hash(&pending);
            match entry.0 {
                PendingItem::Nip46Request {
                    client_name,
                    account,
                    command,
                } => Some((client_name, account, command)),
                _ => None,
            }
        } else {
            None
        }
    }

    pub fn remove(&self, item: &PendingItem) {
        let mut pending = self.pending.write();
        pending.retain(|(entry, _)| entry != item);
        *self.pending_hash.write() = calculate_pending_hash(&pending);
    }

    pub fn compute_pending(&self) -> Result<(), Error> {
        let mypubkey = match GLOBALS.identity.public_key() {
            Some(pk) => pk,
            None => return Ok(()), // nothing pending if no identity
        };

        let now = Unixtime::now().unwrap();
        let t30days = 60 * 60 * 24 * 30;
        let t90days = 60 * 60 * 24 * 90;

        let relay_lists = GLOBALS.storage.find_events(
            &[EventKind::RelayList],
            &[mypubkey],
            None,
            |_| true,
            true,
        )?;

        if relay_lists.is_empty() {
            self.insert(PendingItem::RelayListNeverAdvertised);
        } else {
            self.remove(&PendingItem::RelayListNeverAdvertised); // remove if present

            let stored_relay_list = GLOBALS.storage.load_advertised_relay_list()?;
            let event_relay_list = RelayList::from_event(&relay_lists[0]);

            if stored_relay_list != event_relay_list {
                self.insert(PendingItem::RelayListChangedSinceAdvertised);
            } else {
                self.remove(&PendingItem::RelayListChangedSinceAdvertised); // remove if present

                if relay_lists[0].created_at.0 + t30days < now.0 {
                    self.insert(PendingItem::RelayListNotAdvertisedRecently);
                } else {
                    self.remove(&PendingItem::RelayListNotAdvertisedRecently); // remove if present
                }
            }
        }

        // Check each person list (if out of sync or more than 30 days ago)
        for (list, metadata) in GLOBALS.storage.get_all_person_list_metadata()?.iter() {
            // if never published
            if metadata.event_created_at.0 == 0 {
                self.insert(PendingItem::PersonListNeverPublished(*list));
                continue;
            } else {
                self.remove(&PendingItem::PersonListNeverPublished(*list));
            }

            // If 90 days old, should be re-synced
            if metadata.event_created_at.0 + t90days < now.0 {
                self.insert(PendingItem::PersonListNotPublishedRecently(*list));
                continue;
            } else {
                self.remove(&PendingItem::PersonListNotPublishedRecently(*list));
                // remove if present
            }

            // If mismatched, should be re-synced
            let stored_hash = GLOBALS.storage.hash_person_list(*list)?;
            let last_event_hash = crate::people::hash_person_list_event(*list)?;
            if stored_hash != last_event_hash {
                self.insert(PendingItem::PersonListOutOfSync(*list));
                continue;
            } else {
                self.remove(&PendingItem::PersonListOutOfSync(*list)); // remove if present
            }
        }

        {
            let pending = self.pending.read();
            *self.pending_hash.write() = calculate_pending_hash(&pending);
        }

        Ok(())
    }
}

pub fn start() {
    tracing::info!("Pending checker startup");

    task::spawn(async {
        let mut read_runstate = GLOBALS.read_runstate.clone();
        read_runstate.mark_unchanged();
        if read_runstate.borrow().going_offline() {
            return;
        }

        let sleep = tokio::time::sleep(Duration::from_secs(15));
        tokio::pin!(sleep);

        loop {
            tokio::select! {
                _ = &mut sleep => {
                    sleep.as_mut().reset(Instant::now() + Duration::from_secs(15));
                },
                _ = read_runstate.wait_for(|runstate| runstate.going_offline()) => break,
            }

            match GLOBALS.pending.compute_pending() {
                Ok(()) => {}
                Err(e) => {
                    tracing::error!("{:?}", e);
                    continue;
                }
            };
        }

        tracing::info!("Pending checker shutdown");
    });
}
