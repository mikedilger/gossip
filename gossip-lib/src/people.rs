use crate::comms::ToOverlordMessage;
use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use dashmap::{DashMap, DashSet};
use gossip_relay_picker::Direction;
use image::RgbaImage;
use nostr_types::{
    Event, EventKind, Metadata, PreEvent, PublicKey, RelayUrl, Tag, UncheckedUrl, Unixtime, Url,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task;

/// Person type, aliased to the latest version
pub type Person = crate::storage::types::Person2;

/// PersonList type, aliased to the latest version
pub type PersonList = crate::storage::types::PersonList1;

/// Handles people and remembers what needs to be done for each, such as fetching
/// metadata or avatars.
pub struct People {
    // active person's relays (pull from db as needed)
    active_person: RwLock<Option<PublicKey>>,
    active_persons_write_relays: RwLock<Vec<(RelayUrl, u64)>>,

    // We fetch (with Fetcher), process, and temporarily hold avatars
    // until the UI next asks for them, at which point we remove them
    // and hand them over. This way we can do the work that takes
    // longer and the UI can do as little work as possible.
    avatars_temp: DashMap<PublicKey, RgbaImage>,
    avatars_pending_processing: DashSet<PublicKey>,

    // When we manually ask for updating metadata, we want to recheck
    // the person's NIP-05 when that metadata come in. We remember this here.
    recheck_nip05: DashSet<PublicKey>,

    // People that need metadata, which the UI has asked for. These people
    // might simply not be loaded from the database yet.
    need_metadata: DashSet<PublicKey>,

    // People who we already tried to get their metadata. We only try once
    // per gossip run (this set only grows)
    tried_metadata: DashSet<PublicKey>,

    // Date of the last self-owned contact list we have an event for
    pub last_contact_list_asof: AtomicI64,

    // Size of the last self-owned contact list we have an event for
    pub last_contact_list_size: AtomicUsize,

    // Date of the last self-owned mute list we have an event for
    pub last_mute_list_asof: AtomicI64,

    // Size of the last self-owned mute list we have an event for
    pub last_mute_list_size: AtomicUsize,
}

impl Default for People {
    fn default() -> Self {
        Self::new()
    }
}

impl People {
    pub(crate) fn new() -> People {
        People {
            active_person: RwLock::new(None),
            active_persons_write_relays: RwLock::new(vec![]),
            avatars_temp: DashMap::new(),
            avatars_pending_processing: DashSet::new(),
            recheck_nip05: DashSet::new(),
            need_metadata: DashSet::new(),
            tried_metadata: DashSet::new(),
            last_contact_list_asof: AtomicI64::new(0),
            last_contact_list_size: AtomicUsize::new(0),
            last_mute_list_asof: AtomicI64::new(0),
            last_mute_list_size: AtomicUsize::new(0),
        }
    }

    // Start the periodic task management
    pub(crate) fn start() {
        if let Some(pk) = GLOBALS.signer.public_key() {
            // Load our contact list from the database in order to populate
            // last_contact_list_asof and last_contact_list_size
            if let Ok(Some(event)) = GLOBALS
                .storage
                .get_replaceable_event(pk, EventKind::ContactList)
            {
                if event.created_at.0
                    > GLOBALS
                        .people
                        .last_contact_list_asof
                        .load(Ordering::Relaxed)
                {
                    GLOBALS
                        .people
                        .last_contact_list_asof
                        .store(event.created_at.0, Ordering::Relaxed);
                    let size = event
                        .tags
                        .iter()
                        .filter(|t| matches!(t, Tag::Pubkey { .. }))
                        .count();
                    GLOBALS
                        .people
                        .last_contact_list_size
                        .store(size, Ordering::Relaxed);
                }
            }

            // Load our mute list from the database in order to populate
            // last_mute_list_asof and last_mute_list_size
            if let Ok(Some(event)) = GLOBALS
                .storage
                .get_replaceable_event(pk, EventKind::MuteList)
            {
                if event.created_at.0 > GLOBALS.people.last_mute_list_asof.load(Ordering::Relaxed) {
                    GLOBALS
                        .people
                        .last_mute_list_asof
                        .store(event.created_at.0, Ordering::Relaxed);
                    let size = event
                        .tags
                        .iter()
                        .filter(|t| matches!(t, Tag::Pubkey { .. }))
                        .count();
                    GLOBALS
                        .people
                        .last_mute_list_size
                        .store(size, Ordering::Relaxed);
                }
            }
        }

        task::spawn(async {
            loop {
                let fetch_metadata_looptime_ms =
                    GLOBALS.storage.read_setting_fetcher_metadata_looptime_ms();

                // Every 3 seconds...
                tokio::time::sleep(Duration::from_millis(fetch_metadata_looptime_ms)).await;

                // We fetch needed metadata
                GLOBALS.people.maybe_fetch_metadata().await;

                // And we check for shutdown condition
                if GLOBALS.shutting_down.load(Ordering::Relaxed) {
                    break;
                }
            }
        });
    }

    /// Get all the pubkeys that the user follows
    pub fn get_followed_pubkeys(&self) -> Vec<PublicKey> {
        // We subscribe to all people in all lists.
        // This is no longer synonomous with the ContactList list
        match GLOBALS.storage.get_people_in_all_followed_lists() {
            Ok(people) => people,
            Err(e) => {
                tracing::error!("{}", e);
                vec![]
            }
        }
    }

    /// Get all the pubkeys that the user mutes
    pub fn get_muted_pubkeys(&self) -> Vec<PublicKey> {
        match GLOBALS.storage.get_people_in_list(PersonList::Muted, None) {
            Ok(list) => list,
            Err(e) => {
                tracing::error!("{}", e);
                vec![]
            }
        }
    }

    /// Is the given pubkey followed?
    pub fn is_followed(&self, pubkey: &PublicKey) -> bool {
        self.get_followed_pubkeys().contains(pubkey)
    }

    /// Get all the pubkeys that need relay lists (from the given set)
    pub fn get_followed_pubkeys_needing_relay_lists(
        &self,
        among_these: &[PublicKey],
    ) -> Vec<PublicKey> {
        let stale = Unixtime::now().unwrap().0
            - 60 * 60
                * GLOBALS
                    .storage
                    .read_setting_relay_list_becomes_stale_hours() as i64;

        if let Ok(vec) = GLOBALS.storage.filter_people(|p| {
            p.is_in_list(PersonList::Followed)
                && p.relay_list_last_received < stale
                && among_these.contains(&p.pubkey)
        }) {
            vec.iter().map(|p| p.pubkey).collect()
        } else {
            vec![]
        }
    }

    /// Create person record for this pubkey, if missing
    pub fn create_if_missing(&self, pubkey: PublicKey) {
        if let Err(e) = self.create_all_if_missing(&[pubkey]) {
            tracing::error!("{}", e);
        }
    }

    /// Create person records for these pubkeys, if missing
    pub fn create_all_if_missing(&self, pubkeys: &[PublicKey]) -> Result<(), Error> {
        for pubkey in pubkeys {
            GLOBALS.storage.write_person_if_missing(pubkey, None)?;
        }

        Ok(())
    }

    /// If this person doesn't have metadata, and we are automatically fetching
    /// metadata, then add this person to the list of people that need metadata.
    pub fn person_of_interest(&self, pubkey: PublicKey) {
        // Don't get metadata if disabled
        if !GLOBALS.storage.read_setting_automatically_fetch_metadata() {
            return;
        }

        // Don't try over and over. We try just once per gossip run.
        if self.tried_metadata.contains(&pubkey) {
            return;
        }

        match GLOBALS.storage.read_person(&pubkey) {
            Ok(Some(person)) => {
                // We need metadata if it is missing or old
                let need = {
                    // Metadata refresh interval
                    let now = Unixtime::now().unwrap();
                    let stale = Duration::from_secs(
                        60 * 60 * GLOBALS.storage.read_setting_metadata_becomes_stale_hours(),
                    );
                    person.metadata_created_at.is_none()
                        || person.metadata_last_received < (now - stale).0
                };
                if !need {
                    return;
                }

                // Record that we need it.
                // the periodic task will take care of it.
                if !self.need_metadata.contains(&pubkey) {
                    self.need_metadata.insert(pubkey);
                }
            }
            _ => {
                // Trigger a future create and load
                self.create_if_missing(pubkey);

                // Don't load metadata now, we may have it on disk and get
                // it from the future load.
            }
        }
    }

    /// This is run periodically. It checks the database first, only then does it
    /// ask the overlord to update the metadata from the relays.
    async fn maybe_fetch_metadata(&self) {
        let mut verified_need: Vec<PublicKey> = Vec::new();

        // Take from self.need_metadata;
        let mut need_metadata: Vec<PublicKey> = self
            .need_metadata
            .iter()
            .map(|refmulti| refmulti.key().to_owned())
            .collect();
        self.need_metadata.clear();

        if !need_metadata.is_empty() {
            tracing::debug!("Periodic metadata fetch for {} people", need_metadata.len());
        }

        for pubkey in need_metadata.drain(..) {
            tracing::debug!("Seeking metadata for {}", pubkey.as_hex_string());
            verified_need.push(pubkey);
            self.tried_metadata.insert(pubkey);
        }

        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::UpdateMetadataInBulk(verified_need));
    }

    pub(crate) fn recheck_nip05_on_update_metadata(&self, pubkey: &PublicKey) {
        self.recheck_nip05.insert(pubkey.to_owned());
    }

    pub(crate) async fn update_metadata(
        &self,
        pubkey: &PublicKey,
        metadata: Metadata,
        asof: Unixtime,
    ) -> Result<(), Error> {
        // Sync in from database first
        self.create_all_if_missing(&[*pubkey])?;

        let now = Unixtime::now().unwrap();

        // Copy the person
        let mut person = GLOBALS
            .storage
            .read_person(pubkey)?
            .unwrap_or(Person::new(pubkey.to_owned()));

        // Update metadata_last_received, even if we don't update the metadata
        person.metadata_last_received = now.0;
        GLOBALS.storage.write_person(&person, None)?;

        // Remove from the list of people that need metadata
        self.need_metadata.remove(pubkey);

        // Determine whether it is fresh
        let fresh = match person.metadata_created_at {
            Some(metadata_created_at) => asof.0 > metadata_created_at,
            None => true,
        };

        if fresh {
            let nip05_changed = if let Some(md) = &person.metadata {
                metadata.nip05 != md.nip05.clone()
            } else {
                metadata.nip05.is_some()
            };

            // Update person in the map, and the local variable
            person.metadata = Some(metadata);
            person.metadata_created_at = Some(asof.0);
            if nip05_changed {
                person.nip05_valid = false; // changed, so reset to invalid
                person.nip05_last_checked = None; // we haven't checked this one yet
            }
            GLOBALS.storage.write_person(&person, None)?;
            GLOBALS.ui_people_to_invalidate.write().push(*pubkey);
        }

        // Remove from failed avatars list so the UI will try to fetch the avatar again if missing
        GLOBALS.failed_avatars.write().await.remove(pubkey);

        // Only if they have a nip05 dns id set
        if matches!(
            person,
            Person {
                metadata: Some(Metadata { nip05: Some(_), .. }),
                ..
            }
        ) {
            // Recheck nip05 every day if invalid, and every two weeks if valid

            let recheck = {
                if self.recheck_nip05.contains(pubkey) {
                    self.recheck_nip05.remove(pubkey);
                    true
                } else if let Some(last) = person.nip05_last_checked {
                    // FIXME make these settings
                    let recheck_duration = if person.nip05_valid {
                        Duration::from_secs(
                            60 * 60
                                * GLOBALS
                                    .storage
                                    .read_setting_nip05_becomes_stale_if_valid_hours(),
                        )
                    } else {
                        Duration::from_secs(
                            60 * GLOBALS
                                .storage
                                .read_setting_nip05_becomes_stale_if_invalid_minutes(),
                        )
                    };
                    Unixtime::now().unwrap() - Unixtime(last as i64) > recheck_duration
                } else {
                    true
                }
            };

            if recheck {
                self.update_nip05_last_checked(person.pubkey).await?;
                task::spawn(async move {
                    if let Err(e) = crate::nip05::validate_nip05(person).await {
                        tracing::error!("{}", e);
                    }
                });
            }
        }

        Ok(())
    }

    /// Get the avatar `RgbaImage` for the person.
    ///
    /// This usually returns None when first called, and eventually returns the image.
    /// Once the image is returned, it will return None ever after, because the image is
    /// moved, not copied.
    ///
    /// FIXME this API is not good for async front ends.
    pub fn get_avatar(
        &self,
        pubkey: &PublicKey,
        rounded: bool,
        avatar_size: u32,
    ) -> Option<RgbaImage> {
        // If we have it, hand it over (we won't need a copy anymore)
        if let Some(th) = self.avatars_temp.remove(pubkey) {
            return Some(th.1);
        }

        // If it failed before, error out now
        if GLOBALS.failed_avatars.blocking_read().contains(pubkey) {
            return None; // cannot recover.
        }

        // If it is pending processing, respond now
        if self.avatars_pending_processing.contains(pubkey) {
            return None; // will recover after processing completes
        }

        // Do not fetch if disabled
        if !GLOBALS.storage.read_setting_load_avatars() {
            return None; // can recover if the setting is switched
        }

        // Get the person this is about
        let person = match GLOBALS.storage.read_person(pubkey) {
            Ok(Some(person)) => person,
            _ => return None, // can recover once the person is loaded
        };

        // Fail permanently if they don't have a picture url
        if person.picture().is_none() {
            // this cannot recover without new metadata
            GLOBALS.failed_avatars.blocking_write().insert(*pubkey);

            return None;
        }

        // Fail permanently if the URL is bad
        let url = UncheckedUrl(person.picture().unwrap().to_string());
        let url = match Url::try_from_unchecked_url(&url) {
            Ok(url) => url,
            Err(_) => {
                // this cannot recover without new metadata
                GLOBALS.failed_avatars.blocking_write().insert(*pubkey);

                return None;
            }
        };

        match GLOBALS.fetcher.try_get(
            &url,
            Duration::from_secs(
                60 * 60 * GLOBALS.storage.read_setting_avatar_becomes_stale_hours(),
            ),
        ) {
            // cache expires in 3 days
            Ok(None) => None,
            Ok(Some(bytes)) => {
                // Finish this later (spawn)
                let apubkey = *pubkey;
                tokio::spawn(async move {
                    let size = avatar_size * 3 // 3x feed size, 1x people page size
                        * GLOBALS
                            .pixels_per_point_times_100
                            .load(Ordering::Relaxed)
                        / 100;

                    match crate::media::load_image_bytes(
                        &bytes, true, // crop square
                        size, // default size,
                        true, // force to that size
                        rounded,
                    ) {
                        Ok(color_image) => {
                            GLOBALS.people.avatars_temp.insert(apubkey, color_image);
                        }
                        Err(_) => {
                            // this cannot recover without new metadata
                            GLOBALS.failed_avatars.write().await.insert(apubkey);
                        }
                    }
                });
                self.avatars_pending_processing.insert(pubkey.to_owned());
                None
            }
            Err(e) => {
                tracing::error!("{}", e);
                // this cannot recover without new metadata
                GLOBALS.failed_avatars.blocking_write().insert(*pubkey);
                None
            }
        }
    }

    /// This lets you start typing a name, and autocomplete the results for tagging
    /// someone in a post.  It returns maximum 10 results.
    pub fn search_people_to_tag(&self, mut text: &str) -> Result<Vec<(String, PublicKey)>, Error> {
        // work with or without the @ symbol:
        if text.starts_with('@') {
            text = &text[1..]
        }
        // normalize case
        let search = String::from(text).to_lowercase();

        // grab all results then sort by score
        let mut results: Vec<(u16, String, PublicKey)> = GLOBALS
            .storage
            .filter_people(|_| true)?
            .iter()
            .filter_map(|person| {
                let mut score = 0u16;
                let mut result_name = String::from("");

                // search for users by name
                if let Some(name) = &person.tag_name() {
                    let matchable = name.to_lowercase();
                    if matchable.starts_with(&search) {
                        score = 300;
                        result_name = name.to_string();
                    }
                    if matchable.contains(&search) {
                        score = 200;
                        result_name = name.to_string();
                    }
                }

                // search for users by nip05 id
                if score == 0 && person.nip05_valid {
                    if let Some(nip05) = &person.nip05().map(|n| n.to_lowercase()) {
                        if nip05.starts_with(&search) {
                            score = 400;
                            result_name = nip05.to_string();
                        }
                        if nip05.contains(&search) {
                            score = 100;
                            result_name = nip05.to_string();
                        }
                    }
                }

                if score > 0 {
                    // if there is not a name, fallback to showing the initial chars of the pubkey,
                    // but this is probably unnecessary and will never happen
                    if result_name.is_empty() {
                        result_name = person.pubkey.as_hex_string();
                    }

                    // bigger names have a higher match chance, but they should be scored lower
                    score -= result_name.len() as u16;

                    return Some((score, result_name, person.pubkey));
                }

                None
            })
            .collect();

        results.sort_by(|a, b| a.0.cmp(&b.0).reverse());
        let max = if results.len() > 10 {
            10
        } else {
            results.len()
        };

        Ok(results[0..max]
            .iter()
            .map(|r| (r.1.to_owned(), r.2.to_owned()))
            .collect())
    }

    pub(crate) async fn generate_contact_list_event(&self) -> Result<Event, Error> {
        let mut p_tags: Vec<Tag> = Vec::new();

        let pubkeys = self.get_followed_pubkeys();

        for pubkey in &pubkeys {
            // Get their petname
            let mut petname: Option<String> = None;
            if let Some(person) = GLOBALS.storage.read_person(pubkey)? {
                petname = person.petname.clone();
            }

            // Get their best relay
            let relays = GLOBALS.storage.get_best_relays(*pubkey, Direction::Write)?;
            let maybeurl = relays.get(0);
            p_tags.push(Tag::Pubkey {
                pubkey: (*pubkey).into(),
                recommended_relay_url: maybeurl.map(|(u, _)| u.to_unchecked_url()),
                petname,
                trailing: Vec::new(),
            });
        }

        let public_key = match GLOBALS.signer.public_key() {
            Some(pk) => pk,
            None => return Err((ErrorKind::NoPrivateKey, file!(), line!()).into()), // not even a public key
        };

        // Get the content from our latest ContactList.
        // We don't use the data, but we shouldn't clobber it.

        let content = match GLOBALS
            .storage
            .get_replaceable_event(public_key, EventKind::ContactList)?
        {
            Some(c) => c.content,
            None => "".to_owned(),
        };

        let pre_event = PreEvent {
            pubkey: public_key,
            created_at: Unixtime::now().unwrap(),
            kind: EventKind::ContactList,
            tags: p_tags,
            content,
        };

        GLOBALS.signer.sign_preevent(pre_event, None, None)
    }

    pub(crate) async fn generate_mute_list_event(&self) -> Result<Event, Error> {
        let mut p_tags: Vec<Tag> = Vec::new();

        let muted_pubkeys = self.get_muted_pubkeys();

        for muted_pubkey in &muted_pubkeys {
            p_tags.push(Tag::Pubkey {
                pubkey: (*muted_pubkey).into(),
                recommended_relay_url: None,
                petname: None,
                trailing: vec![],
            });
        }

        let public_key = match GLOBALS.signer.public_key() {
            Some(pk) => pk,
            None => return Err((ErrorKind::NoPrivateKey, file!(), line!()).into()), // not even a public key
        };

        // Get the content from our latest MuteList.
        // We don't use the data, but we shouldn't clobber it (it is for private mutes
        // that we have not implemented yet)

        let content = match GLOBALS
            .storage
            .get_replaceable_event(public_key, EventKind::MuteList)?
        {
            Some(c) => c.content,
            None => "".to_owned(),
        };

        let pre_event = PreEvent {
            pubkey: public_key,
            created_at: Unixtime::now().unwrap(),
            kind: EventKind::MuteList,
            tags: p_tags,
            content,
        };

        GLOBALS.signer.sign_preevent(pre_event, None, None)
    }

    /// Follow (or unfollow) the public key
    pub fn follow(&self, pubkey: &PublicKey, follow: bool, public: bool) -> Result<(), Error> {
        let mut txn = GLOBALS.storage.get_write_txn()?;

        if follow {
            GLOBALS.storage.add_person_to_list(
                pubkey,
                PersonList::Followed,
                public,
                Some(&mut txn),
            )?;
        } else {
            GLOBALS.storage.remove_person_from_list(
                pubkey,
                PersonList::Followed,
                Some(&mut txn),
            )?;
        }
        GLOBALS.ui_people_to_invalidate.write().push(*pubkey);

        GLOBALS
            .storage
            .write_last_contact_list_edit(Unixtime::now().unwrap().0, Some(&mut txn))?;

        txn.commit()?;

        Ok(())
    }

    /// Follow all these public keys.
    /// This does not publish any events.
    pub(crate) fn follow_all(
        &self,
        pubkeys: &[PublicKey],
        public: bool,
        merge: bool,
    ) -> Result<(), Error> {
        let mut txn = GLOBALS.storage.get_write_txn()?;

        if !merge {
            GLOBALS
                .storage
                .clear_person_list(PersonList::Followed, Some(&mut txn))?;
        }

        for pubkey in pubkeys {
            GLOBALS.storage.add_person_to_list(
                pubkey,
                PersonList::Followed,
                public,
                Some(&mut txn),
            )?;
            GLOBALS.ui_people_to_invalidate.write().push(*pubkey);
        }

        GLOBALS
            .storage
            .write_last_contact_list_edit(Unixtime::now().unwrap().0, Some(&mut txn))?;

        txn.commit()?;

        // Add the people to the relay_picker for picking
        for pubkey in pubkeys.iter() {
            GLOBALS.relay_picker.add_someone(pubkey.to_owned())?;
        }

        Ok(())
    }

    /// Empty the following list.
    /// This does not publish any events.
    pub(crate) fn follow_none(&self) -> Result<(), Error> {
        let mut txn = GLOBALS.storage.get_write_txn()?;

        GLOBALS
            .storage
            .clear_person_list(PersonList::Followed, Some(&mut txn))?;
        GLOBALS
            .storage
            .write_last_contact_list_edit(Unixtime::now().unwrap().0, Some(&mut txn))?;

        txn.commit()?;

        GLOBALS.ui_invalidate_all.store(false, Ordering::Relaxed);

        Ok(())
    }

    /// Empty the mute list
    /// This does not publish any events
    pub(crate) fn clear_mute_list(&self) -> Result<(), Error> {
        let mut txn = GLOBALS.storage.get_write_txn()?;

        GLOBALS
            .storage
            .clear_person_list(PersonList::Muted, Some(&mut txn))?;
        GLOBALS
            .storage
            .write_last_mute_list_edit(Unixtime::now().unwrap().0, Some(&mut txn))?;

        txn.commit()?;

        GLOBALS.ui_invalidate_all.store(false, Ordering::Relaxed);

        Ok(())
    }

    /// Mute (or unmute) a public key
    pub fn mute(&self, pubkey: &PublicKey, mute: bool, public: bool) -> Result<(), Error> {
        let mut txn = GLOBALS.storage.get_write_txn()?;

        if mute {
            if let Some(pk) = GLOBALS.signer.public_key() {
                if pk == *pubkey {
                    return Err(ErrorKind::General("You cannot mute yourself".to_owned()).into());
                }
            }

            GLOBALS.storage.add_person_to_list(
                pubkey,
                PersonList::Muted,
                public,
                Some(&mut txn),
            )?;
        } else {
            GLOBALS
                .storage
                .remove_person_from_list(pubkey, PersonList::Muted, Some(&mut txn))?;
        }

        GLOBALS
            .storage
            .write_last_mute_list_edit(Unixtime::now().unwrap().0, Some(&mut txn))?;

        txn.commit()?;

        GLOBALS.ui_people_to_invalidate.write().push(*pubkey);

        Ok(())
    }

    pub(crate) fn mute_all(
        &self,
        pubkeys: &[PublicKey],
        merge: bool,
        public: bool,
    ) -> Result<(), Error> {
        let mut txn = GLOBALS.storage.get_write_txn()?;

        if !merge {
            GLOBALS
                .storage
                .clear_person_list(PersonList::Muted, Some(&mut txn))?;
        }

        for pubkey in pubkeys {
            GLOBALS.storage.add_person_to_list(
                pubkey,
                PersonList::Muted,
                public,
                Some(&mut txn),
            )?;
            GLOBALS.ui_people_to_invalidate.write().push(*pubkey);
        }

        GLOBALS
            .storage
            .write_last_mute_list_edit(Unixtime::now().unwrap().0, Some(&mut txn))?;

        txn.commit()?;

        Ok(())
    }

    // Returns true if the date passed in is newer than what we already had
    pub(crate) async fn update_relay_list_stamps(
        &self,
        pubkey: PublicKey,
        created_at: i64,
    ) -> Result<bool, Error> {
        let now = Unixtime::now().unwrap().0;

        let mut retval = false;

        let mut person = match GLOBALS.storage.read_person(&pubkey)? {
            Some(person) => person,
            None => Person::new(pubkey),
        };

        person.relay_list_last_received = now;
        if let Some(old_at) = person.relay_list_created_at {
            if created_at < old_at {
                retval = false;
            } else {
                person.relay_list_created_at = Some(created_at);
            }
        } else {
            person.relay_list_created_at = Some(created_at);
        }

        GLOBALS.storage.write_person(&person, None)?;

        Ok(retval)
    }

    pub(crate) async fn update_nip05_last_checked(&self, pubkey: PublicKey) -> Result<(), Error> {
        let now = Unixtime::now().unwrap().0;

        if let Some(mut person) = GLOBALS.storage.read_person(&pubkey)? {
            person.nip05_last_checked = Some(now as u64);
            GLOBALS.storage.write_person(&person, None)?;
        }

        Ok(())
    }

    pub(crate) async fn upsert_nip05_validity(
        &self,
        pubkey: &PublicKey,
        nip05: Option<String>,
        nip05_valid: bool,
        nip05_last_checked: u64,
    ) -> Result<(), Error> {
        // Update memory
        if let Some(mut person) = GLOBALS.storage.read_person(pubkey)? {
            if let Some(metadata) = &mut person.metadata {
                metadata.nip05 = nip05
            } else {
                let mut metadata = Metadata::new();
                metadata.nip05 = nip05;
                person.metadata = Some(metadata);
            }
            person.nip05_valid = nip05_valid;
            person.nip05_last_checked = Some(nip05_last_checked);

            GLOBALS.storage.write_person(&person, None)?;
            GLOBALS.ui_people_to_invalidate.write().push(*pubkey);
        }

        Ok(())
    }

    pub(crate) async fn set_active_person(&self, pubkey: PublicKey) -> Result<(), Error> {
        // Set the active person
        *self.active_person.write().await = Some(pubkey);

        // Load their relays
        let best_relays = GLOBALS.storage.get_best_relays(pubkey, Direction::Write)?;
        *self.active_persons_write_relays.write().await = best_relays;

        Ok(())
    }

    pub fn get_active_person(&self) -> Option<PublicKey> {
        *self.active_person.blocking_read()
    }

    pub fn get_active_person_write_relays(&self) -> Vec<(RelayUrl, u64)> {
        self.active_persons_write_relays.blocking_read().clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Nip05Patch {
    nip05: Option<String>,
}
