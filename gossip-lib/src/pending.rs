use crate::comms::RelayJob;
use crate::error::Error;
use crate::globals::GLOBALS;
use crate::nip46::ParsedCommand;
use crate::people::PersonList;
use crate::relay::Relay;
use crate::storage::Storage;
use nostr_types::{EventKind, Filter, PublicKey, PublicKeyHex, RelayList, RelayUrl, Unixtime};
use parking_lot::RwLock as PRwLock;
use parking_lot::RwLockReadGuard as PRwLockReadGuard;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

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
    NeedReadRelays,
    NeedWriteRelays,
    NeedDiscoverRelays,
    NeedDMRelays,
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

        let now = Unixtime::now();
        let t30days = 60 * 60 * 24 * 30;
        let t90days = 60 * 60 * 24 * 90;

        let pkh: PublicKeyHex = mypubkey.into();

        let mut filter = Filter::new();
        filter.add_author(&pkh);
        filter.kinds = vec![EventKind::RelayList];
        let relay_lists = GLOBALS.storage.find_events_by_filter(&filter, |_| true)?;
        filter.kinds = vec![EventKind::DmRelayList];
        let dm_relay_lists = GLOBALS.storage.find_events_by_filter(&filter, |_| true)?;

        if relay_lists.is_empty() && dm_relay_lists.is_empty() {
            self.insert(PendingItem::RelayListNeverAdvertised);
        } else {
            self.remove(&PendingItem::RelayListNeverAdvertised); // remove if present

            let stored_relay_list = GLOBALS.storage.load_effective_public_relay_list()?;
            let event_relay_list = RelayList::from_event(&relay_lists[0]);

            let stored_dm_relays = {
                let mut relays = Relay::choose_relay_urls(Relay::DM, |_| true)?;
                relays.sort();
                relays
            };
            let event_dm_relays = {
                let mut relays: Vec<RelayUrl> = Vec::new();
                if ! dm_relay_lists.is_empty() {
                    for tag in dm_relay_lists[0].tags.iter() {
                        if tag.tagname() == "relay" {
                            if let Ok(relay_url) = RelayUrl::try_from_str(tag.value()) {
                                // Don't use banned relay URLs
                                if !Storage::url_is_banned(&relay_url) {
                                    relays.push(relay_url);
                                }
                            }
                        }
                    }
                    relays.sort();
                }
                relays
            };

            if stored_relay_list != event_relay_list || stored_dm_relays != event_dm_relays {
                self.insert(PendingItem::RelayListChangedSinceAdvertised);
            } else {
                self.remove(&PendingItem::RelayListChangedSinceAdvertised); // remove if present

                // We could also check person.dm_relay_list_created_at, but probably not needed
                // as they both publish together.

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

            // If mismatched, should be re-synced
            let stored_hash = GLOBALS.storage.hash_person_list(*list)?;
            let last_event_hash = crate::people::hash_person_list_event(*list)?;
            if stored_hash != last_event_hash {
                self.insert(PendingItem::PersonListOutOfSync(*list));
                continue;
            } else {
                self.remove(&PendingItem::PersonListOutOfSync(*list)); // remove if present
            }

            // If 90 days old, should be re-synced
            if metadata.event_created_at.0 + t90days < now.0 {
                self.insert(PendingItem::PersonListNotPublishedRecently(*list));
                continue;
            } else {
                self.remove(&PendingItem::PersonListNotPublishedRecently(*list));
                // remove if present
            }
        }

        let relay_urls = Relay::choose_relay_urls(Relay::READ, |_| true)?;
        if relay_urls.is_empty() {
            self.insert(PendingItem::NeedReadRelays);
        } else {
            self.remove(&PendingItem::NeedReadRelays);
        }

        let relay_urls = Relay::choose_relay_urls(Relay::WRITE, |_| true)?;
        if relay_urls.is_empty() {
            self.insert(PendingItem::NeedWriteRelays);
        } else {
            self.remove(&PendingItem::NeedWriteRelays);
        }

        let relay_urls = Relay::choose_relay_urls(Relay::DISCOVER, |_| true)?;
        if relay_urls.is_empty() {
            self.insert(PendingItem::NeedDiscoverRelays);
        } else {
            self.remove(&PendingItem::NeedDiscoverRelays);
        }

        let relay_urls = Relay::choose_relay_urls(Relay::DM, |_| true)?;
        if relay_urls.is_empty() {
            self.insert(PendingItem::NeedDMRelays);
        } else {
            self.remove(&PendingItem::NeedDMRelays);
        }

        {
            let pending = self.pending.read();
            *self.pending_hash.write() = calculate_pending_hash(&pending);
        }

        Ok(())
    }
}
