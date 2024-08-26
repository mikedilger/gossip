use crate::comms::ToOverlordMessage;
use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::misc::{Freshness, Private};
use crate::relay;
use crate::storage::{PersonTable, Table};
use dashmap::{DashMap, DashSet};
use image::RgbaImage;
use nostr_types::{
    ContentEncryptionAlgorithm, Event, EventKind, Metadata, PreEvent, PublicKey, RelayUrl, Tag,
    UncheckedUrl, Unixtime, Url,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task;

/// Person type, aliased to the latest version
pub type Person = crate::storage::types::Person3;

/// PersonList type, aliased to the latest version
pub type PersonList = crate::storage::types::PersonList1;

/// PersonListMetadata type, aliased to the latest version
pub type PersonListMetadata = crate::storage::types::PersonListMetadata3;

/// Handles people and remembers what needs to be done for each, such as fetching
/// metadata or avatars.
pub struct People {
    // active person's relays (pull from db as needed)
    active_person: RwLock<Option<PublicKey>>,
    active_persons_write_relays: RwLock<Vec<RelayUrl>>,
    active_persons_dm_relays: RwLock<Vec<RelayUrl>>,

    // We fetch (with Fetcher), process, and temporarily hold avatars
    // until the UI next asks for them, at which point we remove them
    // and hand them over. This way we can do the work that takes
    // longer and the UI can do as little work as possible.
    avatars_temp: DashMap<PublicKey, RgbaImage>,
    avatars_pending_processing: DashSet<PublicKey>,

    // When we manually ask for updating metadata, we want to recheck
    // the person's NIP-05 when that metadata come in. We remember this here.
    recheck_nip05: DashSet<PublicKey>,

    // People of interest that the UI is showing, whose metadata should
    // be updated if it is stale.
    people_of_interest: DashSet<PublicKey>,

    // Metadata fetches in progress. Once these get too old we can remove them
    // and consider them to have failed.
    // This only relates to the Metadata event, not subsequent avatar or nip05
    // loads.
    fetching_metadata: DashMap<PublicKey, Unixtime>,
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
            active_persons_dm_relays: RwLock::new(vec![]),
            avatars_temp: DashMap::new(),
            avatars_pending_processing: DashSet::new(),
            recheck_nip05: DashSet::new(),
            people_of_interest: DashSet::new(),
            fetching_metadata: DashMap::new(),
        }
    }

    /// Get all the pubkeys that the user subscribes to in any list
    /// (We also force the current user into this list)
    pub fn get_subscribed_pubkeys(&self) -> Vec<PublicKey> {
        // We subscribe to all people in all lists.
        // This is no longer synonymous with the ContactList list
        match GLOBALS.db().get_people_in_all_followed_lists() {
            Ok(mut people) => {
                if let Some(pk) = GLOBALS.identity.public_key() {
                    if !people.contains(&pk) {
                        people.push(pk);
                    }
                }
                people
            }
            Err(e) => {
                tracing::error!("{}", e);
                vec![]
            }
        }
    }

    /// Is the person in the list? (returns false on error)
    #[inline]
    pub fn is_person_in_list(&self, pubkey: &PublicKey, list: PersonList) -> bool {
        GLOBALS
            .db()
            .is_person_in_list(pubkey, list)
            .unwrap_or(false)
    }

    /// Get all the pubkeys that need relay lists (from the given set)
    pub fn get_subscribed_pubkeys_needing_relay_lists(&self) -> Vec<PublicKey> {
        let stale = Unixtime::now().0
            - 60 * GLOBALS.db().read_setting_relay_list_becomes_stale_minutes() as i64;

        if let Ok(vec) = PersonTable::filter_records(|p| {
            p.is_subscribed_to() && p.relay_list_last_sought < stale
        }) {
            vec.iter().map(|p| p.pubkey).collect()
        } else {
            vec![]
        }
    }

    /// Get if a person needs a relay list
    pub fn person_needs_relay_list(pubkey: PublicKey) -> Freshness {
        let staletime = Unixtime::now().0
            - 60 * GLOBALS.db().read_setting_relay_list_becomes_stale_minutes() as i64;

        match PersonTable::read_record(pubkey, None) {
            Err(_) => Freshness::NeverSought,
            Ok(None) => Freshness::NeverSought,
            Ok(Some(p)) => {
                if p.relay_list_last_sought == 0 {
                    Freshness::NeverSought
                } else if p.relay_list_last_sought < staletime {
                    Freshness::Stale
                } else {
                    Freshness::Fresh
                }
            }
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
            PersonTable::create_record_if_missing(*pubkey, None)?;
        }

        Ok(())
    }

    /// Mark this person as a person who the UI wants fresh metadata for.
    /// maybe_fetch_metadata() will do the processing later on.
    pub fn person_of_interest(&self, pubkey: PublicKey) {
        // Don't set if metadata if disabled
        if !GLOBALS.db().read_setting_automatically_fetch_metadata() {
            return;
        }

        self.people_of_interest.insert(pubkey);
    }

    /// The overlord calls this to indicate that it is fetching metadata
    /// for this person from relays
    pub fn metadata_fetch_initiated(&self, pubkeys: &[PublicKey]) {
        let now = Unixtime::now();
        for pubkey in pubkeys {
            self.fetching_metadata.insert(*pubkey, now);
        }
    }

    /// This is run periodically. It checks the database first, only then does it
    /// ask the overlord to update the metadata from the relays.
    pub(crate) async fn maybe_fetch_metadata(&self) {
        // Take everybody out of self.people_of_interest, into a local var
        let mut people_of_interest: Vec<PublicKey> = self
            .people_of_interest
            .iter()
            .map(|refmulti| refmulti.key().to_owned())
            .collect();
        self.people_of_interest.clear();

        if !people_of_interest.is_empty() {
            tracing::trace!(
                "Periodic metadata check against {} people",
                people_of_interest.len()
            );
        }

        let now = Unixtime::now();
        let stale =
            Duration::from_secs(60 * GLOBALS.db().read_setting_metadata_becomes_stale_minutes());

        let mut verified_need: Vec<PublicKey> = Vec::new();

        for pubkey in people_of_interest.drain(..) {
            // If we already tried fetching_metadata (within the stale period)
            // skip them
            // NOTE: if we tried and it never came in, odds are low that trying
            // again will make any difference. Either the person doesn't have
            // metadata or we don't have their proper relays. So a shorter timeout
            // in this circumstance isn't such a great idea.
            let opt_fetching_asof: Option<Unixtime> =
                self.fetching_metadata.get(&pubkey).map(|r| *r.value());
            if let Some(fetching_asof) = opt_fetching_asof {
                if fetching_asof.0 >= (now - stale).0 {
                    continue;
                } else {
                    // remove stale entry
                    self.fetching_metadata.remove(&pubkey);
                }
            }

            match PersonTable::read_record(pubkey, None) {
                Ok(Some(person)) => {
                    // We need metadata if it is missing or old
                    let need = {
                        // Metadata refresh interval
                        person.metadata_created_at.is_none()
                            || person.metadata_last_received < (now - stale).0
                    };
                    if !need {
                        continue;
                    }

                    tracing::debug!("Seeking metadata for {}", pubkey.as_hex_string());
                    verified_need.push(pubkey);
                }
                _ => {
                    // Trigger a future create and load
                    self.create_if_missing(pubkey);
                    // Don't load metadata now, we may have it on disk and get
                    // it from the future load.
                }
            }
        }

        // This fires off the minions to fetch metadata events
        // When they come in, process.rs handles it by calling
        // GLOBALS.people.update_metadata() [down below]
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::UpdateMetadataInBulk(verified_need));
    }

    pub(crate) fn recheck_nip05_on_update_metadata(&self, pubkey: &PublicKey) {
        self.recheck_nip05.insert(pubkey.to_owned());
    }

    pub(crate) fn update_metadata(
        &self,
        pubkey: &PublicKey,
        metadata: Metadata,
        asof: Unixtime,
    ) -> Result<(), Error> {
        // Remove from fetching metadata (fetch is complete)
        self.fetching_metadata.remove(pubkey);

        // Sync in from database first
        self.create_all_if_missing(&[*pubkey])?;

        let now = Unixtime::now();

        // Copy the person
        let mut person =
            PersonTable::read_record(*pubkey, None)?.unwrap_or(Person::new(pubkey.to_owned()));

        // Update metadata_last_received, even if we don't update the metadata
        person.metadata_last_received = now.0;
        PersonTable::write_record(&mut person, None)?;

        // Determine whether it is fresh
        let fresh = match person.metadata_created_at {
            Some(metadata_created_at) => asof.0 > metadata_created_at,
            None => true,
        };

        if fresh {
            let nip05_changed = if let Some(md) = person.metadata() {
                metadata.nip05 != md.nip05.clone()
            } else {
                metadata.nip05.is_some()
            };

            // Update person in the map, and the local variable
            *person.metadata_mut() = Some(metadata);
            person.metadata_created_at = Some(asof.0);
            if nip05_changed {
                person.nip05_valid = false; // changed, so reset to invalid
                person.nip05_last_checked = None; // we haven't checked this one yet
            }
            PersonTable::write_record(&mut person, None)?;
            GLOBALS.ui_people_to_invalidate.write().push(*pubkey);
        }

        // Remove from failed avatars list so the UI will try to fetch the avatar again if missing
        GLOBALS.failed_avatars.write().remove(pubkey);

        // Only if they have a nip05 dns id set
        if matches!(person.metadata(), Some(Metadata { nip05: Some(_), .. })) {
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
                                    .db()
                                    .read_setting_nip05_becomes_stale_if_valid_hours(),
                        )
                    } else {
                        Duration::from_secs(
                            60 * GLOBALS
                                .db()
                                .read_setting_nip05_becomes_stale_if_invalid_minutes(),
                        )
                    };
                    Unixtime::now() - Unixtime(last as i64) > recheck_duration
                } else {
                    true
                }
            };

            if recheck {
                self.update_nip05_last_checked(person.pubkey)?;
                task::spawn(async move {
                    if let Err(e) = crate::nip05::validate_nip05(person).await {
                        tracing::warn!("{}", e);
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
        if GLOBALS.failed_avatars.read().contains(pubkey) {
            return None; // cannot recover.
        }

        // If it is pending processing, respond now
        if self.avatars_pending_processing.contains(pubkey) {
            return None; // will recover after processing completes
        }

        // Do not fetch if disabled
        if !GLOBALS.db().read_setting_load_avatars() {
            return None; // can recover if the setting is switched
        }

        // Get the person this is about
        let person = match PersonTable::read_record(*pubkey, None) {
            Ok(Some(person)) => person,
            _ => return None, // can recover once the person is loaded
        };

        // Fail permanently if they don't have a picture url
        if person.picture().is_none() {
            // this cannot recover without new metadata
            GLOBALS.failed_avatars.write().insert(*pubkey);

            return None;
        }

        // Fail permanently if the URL is bad
        let url = UncheckedUrl(person.picture().unwrap().to_string());
        let url = match Url::try_from_unchecked_url(&url) {
            Ok(url) => url,
            Err(_) => {
                // this cannot recover without new metadata
                GLOBALS.failed_avatars.write().insert(*pubkey);

                return None;
            }
        };

        match GLOBALS.fetcher.try_get(
            &url,
            Duration::from_secs(60 * 60 * GLOBALS.db().read_setting_avatar_becomes_stale_hours()),
            false,
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
                            GLOBALS.failed_avatars.write().insert(apubkey);
                        }
                    }
                });
                self.avatars_pending_processing.insert(pubkey.to_owned());
                None
            }
            Err(e) => {
                tracing::error!("{}", e);
                // this cannot recover without new metadata
                GLOBALS.failed_avatars.write().insert(*pubkey);
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
        let mut results: Vec<(u16, String, PublicKey)> = PersonTable::filter_records(|_| true)?
            .iter()
            .filter_map(|person| {
                let mut score = 0u16;
                let mut result_name = String::from("");

                // search for users by name
                let name = person.best_name();
                let matchable = name.to_lowercase();
                if matchable.starts_with(&search) {
                    score = 300;
                    result_name = name.to_string();
                }
                if matchable.contains(&search) {
                    score = 200;
                    result_name = name.to_string();
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

    pub(crate) async fn generate_person_list_event(
        &self,
        person_list: PersonList,
    ) -> Result<Event, Error> {
        if !GLOBALS.identity.is_unlocked() {
            return Err((ErrorKind::NoPrivateKey, file!(), line!()).into());
        }

        // Get the personlist metadata (dtag, etc)
        let metadata = match GLOBALS.db().get_person_list_metadata(person_list)? {
            Some(m) => m,
            None => return Err(ErrorKind::ListNotFound.into()),
        };

        let my_pubkey = GLOBALS.identity.public_key().unwrap();

        // Read the person list
        let people = GLOBALS.db().get_people_in_list(person_list)?;

        // Determine the event kind
        let kind = match person_list {
            PersonList::Followed => EventKind::ContactList,
            PersonList::Muted => EventKind::MuteList,
            PersonList::Custom(_) => EventKind::FollowSets,
        };

        // Load most recent existing event, if any
        let existing_event: Option<Event> = match kind {
            EventKind::ContactList | EventKind::MuteList => {
                // We fetch for ContactList to preserve the contents
                // We fetch for MuteList to preserve 't', 'e', and "word" tags
                GLOBALS.db().get_replaceable_event(kind, my_pubkey, "")?
            }
            EventKind::FollowSets => {
                // We fetch for FollowSets to preserve various tags we don't use
                GLOBALS
                    .db()
                    .get_replaceable_event(kind, my_pubkey, &metadata.dtag)?
            }
            _ => None,
        };

        // Get all tags off of the existing event
        // (we use local data to determine public/private, we don't need to remember
        //  where they were in the existing event)
        let old_tags = {
            if let Some(ref event) = existing_event {
                if !event.content.is_empty() && kind != EventKind::ContactList {
                    let decrypted_content = GLOBALS.identity.decrypt(&my_pubkey, &event.content)?;
                    let mut tags: Vec<Tag> = serde_json::from_str(&decrypted_content)?;
                    tags.extend(event.tags.clone());
                    tags
                } else {
                    event.tags.clone()
                }
            } else {
                vec![]
            }
        };

        let mut public_tags: Vec<Tag> = Vec::new();
        let mut private_tags: Vec<Tag> = Vec::new();

        // If FollowSets
        if matches!(person_list, PersonList::Custom(_)) {
            // Add d-tag
            public_tags.push(Tag::new_identifier(metadata.dtag.clone()));

            // Add title if using FollowSets
            let title = Tag::new_title(metadata.title.clone());

            if *metadata.private {
                private_tags.push(title);
            } else {
                public_tags.push(title);
            }

            // Preserve existing tags that we don't operate on yet
            for t in &old_tags {
                if t.tagname() == "image" || t.tagname() == "description" {
                    if *metadata.private {
                        private_tags.push(t.clone());
                    } else {
                        public_tags.push(t.clone());
                    }
                }
            }
        }

        // If MuteList
        if person_list == PersonList::Muted {
            // Preserve existing tags that we don't operate on yet
            for t in &old_tags {
                match t.tagname() {
                    "t" | "e" => {
                        if *metadata.private {
                            private_tags.push(t.clone());
                        } else {
                            public_tags.push(t.clone());
                        }
                    }
                    "word" => {
                        if *metadata.private {
                            private_tags.push(t.clone());
                        } else {
                            public_tags.push(t.clone());
                        }
                    }
                    _ => (),
                }
            }
        }

        // Add the people
        for (pubkey, mut private) in people.iter() {
            // If entire list is private, then all entries are forced to private
            if *metadata.private {
                private = Private(true);
            }

            // Only include petnames in the ContactList (which is only public people)
            let petname = if kind == EventKind::ContactList {
                if let Some(person) = PersonTable::read_record(*pubkey, None)? {
                    person.petname.clone()
                } else {
                    None
                }
            } else {
                None
            };

            // Only include recommended relay urls in public entries, and not in the mute list
            let recommended_relay_url = {
                if kind != EventKind::MuteList && !private.0 {
                    let relays = relay::get_some_pubkey_outboxes(*pubkey)?;
                    relays.first().map(|u| u.to_unchecked_url())
                } else {
                    None
                }
            };

            let tag = Tag::new_pubkey(*pubkey, recommended_relay_url, petname);
            if *private {
                private_tags.push(tag);
            } else {
                public_tags.push(tag);
            }
        }

        let content = {
            if kind == EventKind::ContactList {
                // Preserve the contents of any existing kind-3 event for use by
                // other clients
                match existing_event {
                    Some(c) => c.content,
                    None => "".to_owned(),
                }
            } else {
                let private_tags_string = serde_json::to_string(&private_tags)?;
                GLOBALS.identity.encrypt(
                    &my_pubkey,
                    &private_tags_string,
                    ContentEncryptionAlgorithm::Nip04,
                )?
            }
        };

        let pre_event = PreEvent {
            pubkey: my_pubkey,
            created_at: Unixtime::now(),
            kind,
            tags: public_tags,
            content,
        };

        GLOBALS.identity.sign_event(pre_event)
    }

    /// Follow (or unfollow) the public key
    pub fn follow(
        &self,
        pubkey: &PublicKey,
        follow: bool,
        list: PersonList,
        private: Private,
    ) -> Result<(), Error> {
        if follow {
            GLOBALS
                .db()
                .add_person_to_list(pubkey, list, private, None)?;

            // Add to the relay picker. If they are already there, it will be ok.
            GLOBALS.relay_picker.add_someone(*pubkey)?;

            // Maybe seek relay list (if needed)
            let seek_relay_list = match Self::person_needs_relay_list(*pubkey) {
                Freshness::NeverSought => true,
                Freshness::Stale => true,
                Freshness::Fresh => false,
            };
            if seek_relay_list {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::SubscribeDiscover(vec![*pubkey], None));
            };
        } else {
            GLOBALS.db().remove_person_from_list(pubkey, list, None)?;

            // Don't remove from relay picker here. They might still be on other
            // lists. Garbage collection will eventually clean it up.
        }

        GLOBALS.ui_people_to_invalidate.write().push(*pubkey);

        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::RefreshScoresAndPickRelays);

        Ok(())
    }

    /// Clear a person list
    pub(crate) fn clear_person_list(&self, list: PersonList) -> Result<(), Error> {
        GLOBALS.db().clear_person_list(list, None)?;
        GLOBALS.ui_invalidate_all.store(false, Ordering::Relaxed);
        Ok(())
    }

    /// Mute (or unmute) a public key
    pub fn mute(&self, pubkey: &PublicKey, mute: bool, private: Private) -> Result<(), Error> {
        let mut txn = GLOBALS.db().get_write_txn()?;

        if mute {
            if let Some(pk) = GLOBALS.identity.public_key() {
                if pk == *pubkey {
                    return Err(ErrorKind::General("You cannot mute yourself".to_owned()).into());
                }
            }

            GLOBALS
                .db()
                .add_person_to_list(pubkey, PersonList::Muted, private, Some(&mut txn))?;
        } else {
            GLOBALS
                .db()
                .remove_person_from_list(pubkey, PersonList::Muted, Some(&mut txn))?;
        }

        if let Some(mut metadata) = GLOBALS.db().get_person_list_metadata(PersonList::Muted)? {
            metadata.last_edit_time = Unixtime::now();
            GLOBALS
                .db()
                .set_person_list_metadata(PersonList::Muted, &metadata, Some(&mut txn))?;
        }

        txn.commit()?;

        GLOBALS.ui_people_to_invalidate.write().push(*pubkey);

        Ok(())
    }

    // Returns true if the date passed in is newer than what we already had
    pub(crate) fn update_relay_list_stamps(
        &self,
        pubkey: PublicKey,
        created_at: i64,
    ) -> Result<bool, Error> {
        let mut retval = false;

        PersonTable::modify(
            pubkey,
            |person| {
                if let Some(old_at) = person.relay_list_created_at {
                    if created_at < old_at {
                        retval = false;
                    } else {
                        retval = true;
                        person.relay_list_created_at = Some(created_at);
                    }
                } else {
                    person.relay_list_created_at = Some(created_at);
                }
            },
            None,
        )?;

        Ok(retval)
    }

    pub(crate) fn update_nip05_last_checked(&self, pubkey: PublicKey) -> Result<(), Error> {
        let now = Unixtime::now().0;

        PersonTable::modify(
            pubkey,
            |person| person.nip05_last_checked = Some(now as u64),
            None,
        )?;

        Ok(())
    }

    pub(crate) fn upsert_nip05_validity(
        &self,
        pubkey: &PublicKey,
        nip05: Option<String>,
        nip05_valid: bool,
        nip05_last_checked: u64,
    ) -> Result<(), Error> {
        // Update memory
        PersonTable::modify(
            *pubkey,
            |person| {
                if let Some(metadata) = person.metadata_mut() {
                    metadata.nip05 = nip05.clone();
                } else {
                    let mut metadata = Metadata::new();
                    metadata.nip05 = nip05.clone();
                    *person.metadata_mut() = Some(metadata);
                }
                person.nip05_valid = nip05_valid;
                person.nip05_last_checked = Some(nip05_last_checked);
                GLOBALS.ui_people_to_invalidate.write().push(*pubkey);
            },
            None,
        )?;

        Ok(())
    }

    pub(crate) async fn set_active_person(&self, pubkey: PublicKey) -> Result<(), Error> {
        // Set the active person
        *self.active_person.write().await = Some(pubkey);

        // Load their relays
        let best_relays = relay::get_all_pubkey_outboxes(pubkey)?;
        *self.active_persons_write_relays.write().await = best_relays;

        // Load their DM relays
        let dm_relays = relay::get_dm_relays(pubkey)?;
        *self.active_persons_dm_relays.write().await = dm_relays;

        Ok(())
    }

    /// DO NOT CALL FROM LIB, ONLY FROM UI
    pub fn get_active_person(&self) -> Option<PublicKey> {
        *self.active_person.blocking_read()
    }

    pub async fn get_active_person_async(&self) -> Option<PublicKey> {
        *self.active_person.read().await
    }

    /// DO NOT CALL FROM LIB, ONLY FROM UI
    pub fn get_active_person_write_relays(&self) -> Vec<RelayUrl> {
        self.active_persons_write_relays.blocking_read().clone()
    }

    /// DO NOT CALL FROM LIB, ONLY FROM UI
    pub fn get_active_person_dm_relays(&self) -> Vec<RelayUrl> {
        self.active_persons_dm_relays.blocking_read().clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Nip05Patch {
    nip05: Option<String>,
}

// Determine PersonList and fetches Metadata, allocating if needed.
// This does NOT update that metadata from the event.
// The bool indicates if the list was freshly allocated
pub(crate) fn fetch_current_personlist_matching_event(
    event: &Event,
) -> Result<(PersonList, PersonListMetadata, bool), Error> {
    let (list, metadata, new) = match event.kind {
        EventKind::ContactList => {
            let list = PersonList::Followed;
            match GLOBALS.db().get_person_list_metadata(list)? {
                Some(md) => (list, md, false),
                None => (list, Default::default(), true),
            }
        }
        EventKind::MuteList => {
            let list = PersonList::Muted;
            match GLOBALS.db().get_person_list_metadata(list)? {
                Some(md) => (list, md, false),
                None => (list, Default::default(), true),
            }
        }
        EventKind::FollowSets => {
            let dtag = match event.parameter() {
                Some(dtag) => dtag,
                None => return Err(ErrorKind::ListEventMissingDtag.into()),
            };
            if let Some((found_list, metadata)) = GLOBALS.db().find_person_list_by_dtag(&dtag)? {
                (found_list, metadata, false)
            } else {
                // Allocate new
                let metadata = PersonListMetadata {
                    dtag,
                    event_created_at: event.created_at,
                    ..Default::default()
                };

                // This is slim metadata.. The caller will fix it.
                let list = GLOBALS.db().allocate_person_list(&metadata, None)?;

                (list, metadata, true)
            }
        }
        _ => {
            // This function does not apply to other event kinds
            return Err(ErrorKind::NotAPersonListEvent.into());
        }
    };

    Ok((list, metadata, new))
}

// as opposed to GLOBALS.db().hash_person_list(list)
pub fn hash_person_list_event(list: PersonList) -> Result<u64, Error> {
    // we cannot do anything without an identity setup first
    let my_pubkey = match GLOBALS.db().read_setting_public_key() {
        Some(pk) => pk,
        None => return Err(ErrorKind::NoPublicKey.into()),
    };

    // Get the metadata of the list, which affects force-private logic
    let metadata = match GLOBALS.db().get_person_list_metadata(list)? {
        Some(m) => m,
        None => return Err(ErrorKind::ListNotFound.into()), // list event not found
    };

    // Load the latest PersonList event from the database
    let maybe_event =
        GLOBALS
            .db()
            .get_replaceable_event(list.event_kind(), my_pubkey, &metadata.dtag)?;

    if let Some(event) = maybe_event {
        // Collect the data in an ordered map
        let mut map: BTreeMap<PublicKey, Private> = BTreeMap::new();

        // Collect public entries
        for tag in &event.tags {
            if let Ok((pubkey, _, _)) = tag.parse_pubkey() {
                map.insert(pubkey, metadata.private);
            }
        }

        // Collect private entries
        if event.kind != EventKind::ContactList && !event.content.is_empty() {
            if GLOBALS.identity.is_unlocked() {
                let decrypted_content = GLOBALS.identity.decrypt(&my_pubkey, &event.content)?;
                let tags: Vec<Tag> = serde_json::from_str(&decrypted_content)?;
                for tag in &tags {
                    if let Ok((pubkey, _, _)) = tag.parse_pubkey() {
                        map.insert(pubkey, Private(true));
                    }
                }
            } else {
                return Err(ErrorKind::NoPrivateKey.into());
            }
        }

        // Hash
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for (person, private) in map.iter() {
            let private = if list == PersonList::Followed {
                Private(false)
            } else {
                *private
            };
            person.hash(&mut hasher);
            private.hash(&mut hasher);
        }

        Ok(hasher.finish())
    } else {
        Ok(0)
    }
}
