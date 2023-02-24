use crate::comms::ToOverlordMessage;
use crate::db::{DbEvent, DbPersonRelay};
use crate::error::Error;
use crate::globals::GLOBALS;
use crate::AVATAR_SIZE;
use dashmap::{DashMap, DashSet};
use eframe::egui::ColorImage;
use egui_extras::image::FitTo;
use gossip_relay_picker::Direction;
use image::imageops::FilterType;
use nostr_types::{
    Event, EventKind, Metadata, PreEvent, PublicKey, PublicKeyHex, RelayUrl, Tag, UncheckedUrl,
    Unixtime, Url,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbPerson {
    pub pubkey: PublicKeyHex,
    pub metadata: Option<Metadata>,
    pub metadata_at: Option<i64>,
    pub nip05_valid: u8,
    pub nip05_last_checked: Option<u64>,
    pub followed: u8,
    pub followed_last_updated: i64,
    pub muted: u8,
    pub relay_list_last_received: i64,
    pub relay_list_created_at: i64,
}

impl DbPerson {
    pub fn new(pubkey: PublicKeyHex) -> DbPerson {
        DbPerson {
            pubkey,
            metadata: None,
            metadata_at: None,
            nip05_valid: 0,
            nip05_last_checked: None,
            followed: 0,
            followed_last_updated: 0,
            muted: 0,
            relay_list_last_received: 0,
            relay_list_created_at: 0,
        }
    }

    pub fn display_name(&self) -> Option<&str> {
        if let Some(md) = &self.metadata {
            if md.other.contains_key("display_name") {
                if let Some(serde_json::Value::String(s)) = md.other.get("display_name") {
                    if !s.is_empty() {
                        return Some(s);
                    }
                }
            }
            md.name.as_deref()
        } else {
            None
        }
    }

    pub fn name(&self) -> Option<&str> {
        if let Some(md) = &self.metadata {
            md.name.as_deref()
        } else {
            None
        }
    }

    pub fn about(&self) -> Option<&str> {
        if let Some(md) = &self.metadata {
            md.about.as_deref()
        } else {
            None
        }
    }

    pub fn picture(&self) -> Option<&str> {
        if let Some(md) = &self.metadata {
            md.picture.as_deref()
        } else {
            None
        }
    }

    pub fn nip05(&self) -> Option<&str> {
        if let Some(md) = &self.metadata {
            md.nip05.as_deref()
        } else {
            None
        }
    }
}

pub struct People {
    people: DashMap<PublicKeyHex, DbPerson>,

    // active person's relays (pull from db as needed)
    active_person: RwLock<Option<PublicKeyHex>>,
    active_persons_write_relays: RwLock<Vec<(RelayUrl, u64)>>,

    // We fetch (with Fetcher), process, and temporarily hold avatars
    // until the UI next asks for them, at which point we remove them
    // and hand them over. This way we can do the work that takes
    // longer and the UI can do as little work as possible.
    avatars_temp: DashMap<PublicKeyHex, ColorImage>,
    avatars_pending_processing: DashSet<PublicKeyHex>,
    avatars_failed: DashSet<PublicKeyHex>,

    // When we manually ask for updating metadata, we want to recheck
    // the person's NIP-05 when that metadata come in. We remember this here.
    recheck_nip05: DashSet<PublicKeyHex>,

    // People that need metadata, which the UI has asked for
    need_metadata: DashSet<PublicKeyHex>,
    // People who we already tried to get their metadata (we do not retry
    // until client restart)
    tried_metadata: DashSet<PublicKeyHex>,

    // Date of the last self-owned contact list we processed
    pub last_contact_list_asof: AtomicI64,
}

impl People {
    pub fn new() -> People {
        People {
            people: DashMap::new(),
            active_person: RwLock::new(None),
            active_persons_write_relays: RwLock::new(vec![]),
            avatars_temp: DashMap::new(),
            avatars_pending_processing: DashSet::new(),
            avatars_failed: DashSet::new(),
            recheck_nip05: DashSet::new(),
            need_metadata: DashSet::new(),
            tried_metadata: DashSet::new(),
            last_contact_list_asof: AtomicI64::new(0),
        }
    }

    // Start the periodic task management
    pub fn start() {
        task::spawn(async {
            loop {
                tokio::time::sleep(Duration::from_millis(1500)).await;

                GLOBALS.people.maybe_fetch_metadata();

                // check for shutdown condition
                if GLOBALS.shutting_down.load(Ordering::Relaxed) {
                    break;
                }
            }
        });
    }

    pub fn get_followed_pubkeys(&self) -> Vec<PublicKeyHex> {
        let mut output: Vec<PublicKeyHex> = Vec::new();
        for person in self
            .people
            .iter()
            .filter_map(|p| if p.followed == 1 { Some(p) } else { None })
        {
            output.push(person.pubkey.clone());
        }
        output
    }

    pub fn get_followed_pubkeys_needing_relay_lists(
        &self,
        among_these: &[PublicKeyHex],
    ) -> Vec<PublicKeyHex> {
        // FIXME make this a setting (8 hours)
        let one_day_ago = Unixtime::now().unwrap().0 - (60 * 60 * 8);
        let mut output: Vec<PublicKeyHex> = Vec::new();
        for person in self.people.iter().filter_map(|p| {
            if p.followed == 1
                && p.relay_list_last_received < one_day_ago
                && among_these.contains(&p.pubkey)
            {
                Some(p)
            } else {
                None
            }
        }) {
            output.push(person.pubkey.clone());
        }
        output
    }

    pub fn create_if_missing_sync(&self, pubkey: PublicKeyHex) {
        task::spawn(async {
            if let Err(e) = GLOBALS.people.create_all_if_missing(&[pubkey]).await {
                tracing::error!("{}", e);
            }
        });
    }

    pub async fn create_all_if_missing(&self, pubkeys: &[PublicKeyHex]) -> Result<(), Error> {
        // Collect the public keys that we don't have already (by checking in memory).
        let pubkeys: Vec<&PublicKeyHex> = pubkeys
            .iter()
            .filter(|pk| !self.people.contains_key(pk))
            .collect();

        if pubkeys.is_empty() {
            return Ok(());
        }

        // Make sure all these people exist in the database
        let mut sql: String = "INSERT OR IGNORE INTO person (pubkey) VALUES ".to_owned();
        sql.push_str(&"(?),".repeat(pubkeys.len()));
        sql.pop(); // remove trailing comma

        let pubkey_strings: Vec<String> = pubkeys.iter().map(|p| p.to_string()).collect();

        task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(&sql)?;
            let mut pos = 1;
            for pk in pubkey_strings.iter() {
                stmt.raw_bind_parameter(pos, pk)?;
                pos += 1;
            }
            stmt.raw_execute()?;
            Ok::<(), Error>(())
        })
        .await??;

        // Now load them from the database (some of them may have had records already)
        let mut loaded_people = Self::fetch_many(&pubkeys).await?;
        for loaded_person in loaded_people.drain(..) {
            let _ = self
                .people
                .insert(loaded_person.pubkey.clone(), loaded_person);
        }

        Ok(())
    }

    // If this person doesn't have metadata, and we are automatically fetching
    // metadata, then add this person to the list of people that need metadata.
    pub fn person_of_interest(&self, pubkeyhex: PublicKeyHex) {
        if !GLOBALS
            .settings
            .blocking_read()
            .automatically_fetch_metadata
        {
            return;
        }

        if self.tried_metadata.contains(&pubkeyhex) {
            return;
        }

        match self.people.get(&pubkeyhex) {
            Some(person) => {
                if person.metadata_at.is_none() {
                    if !self.need_metadata.contains(&pubkeyhex) {
                        self.need_metadata.insert(pubkeyhex.clone());
                        tracing::debug!("Need metadata for {} pubkeys", self.need_metadata.len());
                    }
                }
            }
            None => {
                self.create_if_missing_sync(pubkeyhex.clone());
                if !self.need_metadata.contains(&pubkeyhex) {
                    self.need_metadata.insert(pubkeyhex.clone());
                    tracing::debug!("Need metadata for {} pubkeys", self.need_metadata.len());
                }
            }
        }
    }

    // This is run periodically
    pub fn maybe_fetch_metadata(&self) {
        if self.need_metadata.is_empty() {
            return;
        }

        let mut maybe_pubkey: Option<PublicKeyHex> = None;

        if let Some(refmulti) = self.need_metadata.iter().next() {
            maybe_pubkey = Some(refmulti.key().to_owned());
        }

        if let Some(pubkey) = maybe_pubkey {
            self.need_metadata.remove(&pubkey);
            self.tried_metadata.insert(pubkey.clone());
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::UpdateMetadata(pubkey));
        }
    }

    pub fn recheck_nip05_on_update_metadata(&self, pubkeyhex: &PublicKeyHex) {
        self.recheck_nip05.insert(pubkeyhex.to_owned());
    }

    pub async fn update_metadata(
        &self,
        pubkeyhex: &PublicKeyHex,
        metadata: Metadata,
        asof: Unixtime,
    ) -> Result<(), Error> {
        // Sync in from database first
        self.create_all_if_missing(&[pubkeyhex.to_owned()]).await?;

        let person = self.people.get(pubkeyhex).unwrap().to_owned();

        // Remove from list of people that need metadata
        self.need_metadata.remove(pubkeyhex);

        // Determine whether to update it
        let mut doit = person.metadata_at.is_none();
        if let Some(metadata_at) = person.metadata_at {
            if asof.0 > metadata_at {
                doit = true;
            }
        }

        if doit {
            let nip05_changed = if let Some(md) = &person.metadata {
                metadata.nip05 != md.nip05.clone()
            } else {
                metadata.nip05.is_some()
            };

            {
                let mut person_mut = self.people.get_mut(pubkeyhex).unwrap();
                person_mut.metadata = Some(metadata);
                person_mut.metadata_at = Some(asof.0);
                if nip05_changed {
                    person_mut.nip05_valid = 0; // changed, so reset to invalid
                    person_mut.nip05_last_checked = None; // we haven't checked this one yet
                }
            }

            // Update the database
            let pubkeyhex2 = pubkeyhex.to_owned();
            let person = person.clone();
            task::spawn_blocking(move || {
                let maybe_db = GLOBALS.db.blocking_lock();
                let db = maybe_db.as_ref().unwrap();

                let metadata_json: Option<String> = if let Some(md) = &person.metadata {
                    Some(serde_json::to_string(md)?)
                } else {
                    None
                };

                let mut stmt = db.prepare(
                    "UPDATE person SET metadata=?, metadata_at=?, nip05_valid=?, nip05_last_checked=? WHERE pubkey=?"
                )?;
                stmt.execute((
                    &metadata_json,
                    &person.metadata_at,
                    &person.nip05_valid,
                    &person.nip05_last_checked,
                    pubkeyhex2.as_str(),
                ))?;
                Ok::<(), Error>(())
            })
                .await??;
        }

        // Remove from failed avatars list so the UI will try to fetch the avatar again if missing
        GLOBALS.failed_avatars.write().await.remove(pubkeyhex);

        // Only if they have a nip05 dns id set
        if matches!(
            person,
            DbPerson {
                metadata: Some(Metadata { nip05: Some(_), .. }),
                ..
            }
        ) {
            // Recheck nip05 every day if invalid, and every two weeks if valid

            let recheck = {
                if self.recheck_nip05.contains(pubkeyhex) {
                    self.recheck_nip05.remove(pubkeyhex);
                    true
                } else if let Some(last) = person.nip05_last_checked {
                    // FIXME make these settings
                    let recheck_duration = if person.nip05_valid > 0 {
                        Duration::from_secs(60 * 60 * 24 * 14)
                    } else {
                        Duration::from_secs(60 * 60 * 24)
                    };
                    Unixtime::now().unwrap() - Unixtime(last as i64) > recheck_duration
                } else {
                    true
                }
            };

            if recheck {
                self.update_nip05_last_checked(person.pubkey.clone())
                    .await?;
                task::spawn(async move {
                    if let Err(e) = crate::nip05::validate_nip05(person).await {
                        tracing::error!("{}", e);
                    }
                });
            }
        }

        Ok(())
    }

    pub async fn load_all_followed(&self) -> Result<(), Error> {
        if !self.people.is_empty() {
            return Err(Error::Internal(
                "load_all_followed should only be called before people is otherwise used."
                    .to_owned(),
            ));
        }

        // NOTE: We also load all muted people, so that we can render the list of people
        //       who are muted, so they can be found and unmuted as necessary.

        let sql = "SELECT pubkey, metadata, metadata_at, nip05_valid, nip05_last_checked, \
                   followed, followed_last_updated, muted, relay_list_last_received, \
                   relay_list_created_at \
                   FROM person WHERE followed=1 OR muted=1"
            .to_owned();

        let output: Result<Vec<DbPerson>, Error> = task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let mut rows = stmt.query([])?;
            let mut output: Vec<DbPerson> = Vec::new();
            while let Some(row) = rows.next()? {
                let metadata_json: Option<String> = row.get(1)?;
                let metadata = match metadata_json {
                    Some(s) => serde_json::from_str(&s)?,
                    None => None,
                };
                let pk: String = row.get(0)?;
                output.push(DbPerson {
                    pubkey: PublicKeyHex::try_from_string(pk)?,
                    metadata,
                    metadata_at: row.get(2)?,
                    nip05_valid: row.get(3)?,
                    nip05_last_checked: row.get(4)?,
                    followed: row.get(5)?,
                    followed_last_updated: row.get(6)?,
                    muted: row.get(7)?,
                    relay_list_last_received: row.get(8)?,
                    relay_list_created_at: row.get(9)?,
                });
            }
            Ok(output)
        })
        .await?;

        for person in output? {
            self.people.insert(person.pubkey.clone(), person);
        }

        Ok(())
    }

    pub fn get(&self, pubkeyhex: &PublicKeyHex) -> Option<DbPerson> {
        if self.people.contains_key(pubkeyhex) {
            self.people.get(pubkeyhex).map(|o| o.value().to_owned())
        } else {
            // We can't get it now, but we can setup a task to do it soon
            let pubkeyhex = pubkeyhex.to_owned();
            tokio::spawn(async move {
                #[allow(clippy::map_entry)]
                if !GLOBALS.people.people.contains_key(&pubkeyhex) {
                    match People::fetch_one(&pubkeyhex).await {
                        Ok(Some(person)) => {
                            let _ = GLOBALS.people.people.insert(pubkeyhex, person);
                        }
                        Err(e) => tracing::error!("{}", e),
                        _ => {}
                    }
                }
            });
            None
        }
    }

    pub fn get_all(&self) -> Vec<DbPerson> {
        let mut v: Vec<DbPerson> = self.people.iter().map(|e| e.value().to_owned()).collect();
        v.sort_by(|a, b| {
            let c = a
                .display_name()
                .map(|s| s.to_lowercase())
                .cmp(&b.display_name().map(|s| s.to_lowercase()));
            if c == std::cmp::Ordering::Equal {
                a.pubkey.cmp(&b.pubkey)
            } else {
                c
            }
        });
        v
    }

    // If returns Err, means you're never going to get it so stop trying.
    pub fn get_avatar(&self, pubkeyhex: &PublicKeyHex) -> Result<Option<ColorImage>, ()> {
        // If we have it, hand it over (we won't need a copy anymore)
        if let Some(th) = self.avatars_temp.remove(pubkeyhex) {
            return Ok(Some(th.1));
        }

        // If it failed before, error out now
        if self.avatars_failed.contains(pubkeyhex) {
            return Err(());
        }

        // If it is pending processing, respond now
        if self.avatars_pending_processing.contains(pubkeyhex) {
            return Ok(None);
        }

        // Get the person this is about
        let person = match self.people.get(pubkeyhex) {
            Some(person) => person,
            None => {
                return Err(());
            }
        };

        // Fail if they don't have a picture url
        // FIXME: we could get metadata that sets this while we are running, so just failing for
        //        the duration of the client isn't quite right. But for now, retrying is taxing.
        if person.picture().is_none() {
            return Err(());
        }

        // FIXME: we could get metadata that sets this while we are running, so just failing for
        //        the duration of the client isn't quite right. But for now, retrying is taxing.
        let url = UncheckedUrl(person.picture().unwrap().to_string());
        let url = match Url::try_from_unchecked_url(&url) {
            Ok(url) => url,
            Err(_) => return Err(()),
        };

        // Do not fetch if disabled
        if !GLOBALS.settings.blocking_read().load_avatars {
            GLOBALS.people.avatars_failed.insert(pubkeyhex.clone());
            return Err(());
        }

        match GLOBALS.fetcher.try_get(url) {
            Ok(None) => Ok(None),
            Ok(Some(bytes)) => {
                // Finish this later (spawn)
                let apubkeyhex = pubkeyhex.to_owned();
                tokio::spawn(async move {
                    let size = AVATAR_SIZE * 3 // 3x feed size, 1x people page size
                        * GLOBALS
                            .pixels_per_point_times_100
                            .load(Ordering::Relaxed)
                        / 100;
                    if let Ok(image) = image::load_from_memory(&bytes) {
                        // Note: we can't use egui_extras::image::load_image_bytes because we
                        // need to force a resize
                        let image = image.resize(size, size, FilterType::CatmullRom); // DynamicImage
                        let image_buffer = image.into_rgba8(); // RgbaImage (ImageBuffer)
                        let color_image = ColorImage::from_rgba_unmultiplied(
                            [
                                image_buffer.width() as usize,
                                image_buffer.height() as usize,
                            ],
                            image_buffer.as_flat_samples().as_slice(),
                        );
                        GLOBALS.people.avatars_temp.insert(apubkeyhex, color_image);
                    } else if let Ok(color_image) = egui_extras::image::load_svg_bytes_with_size(
                        &bytes,
                        FitTo::Size(size, size),
                    ) {
                        GLOBALS.people.avatars_temp.insert(apubkeyhex, color_image);
                    } else {
                        GLOBALS.people.avatars_failed.insert(apubkeyhex.clone());
                    };
                });
                self.avatars_pending_processing.insert(pubkeyhex.to_owned());
                Ok(None)
            }
            Err(e) => {
                tracing::error!("{}", e);
                self.avatars_failed.insert(pubkeyhex.to_owned());
                Err(())
            }
        }
    }

    /// This lets you start typing a name, and autocomplete the results for tagging
    /// someone in a post.  It returns maximum 10 results.
    pub fn search_people_to_tag(&self, mut text: &str) -> Vec<(String, PublicKey)> {
        // work with or without the @ symbol:
        if text.starts_with('@') {
            text = &text[1..]
        }
        // normalize case
        let search = String::from(text).to_lowercase();

        // grab all results then sort by score
        let mut results: Vec<(u16, String, PublicKeyHex)> = self
            .people
            .iter()
            .filter_map(|person| {
                let mut score = 0u16;
                let mut result_name = String::from("");

                // search for users by name
                if let Some(name) = &person.display_name() {
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
                if score == 0 && person.nip05_valid > 0 {
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
                        result_name = person.pubkey.to_string();
                    }

                    // bigger names have a higher match chance, but they should be scored lower
                    score -= result_name.len() as u16;

                    return Some((score, result_name, person.pubkey.clone()));
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
        results[0..max]
            .iter()
            .map(|r| {
                (
                    r.1.to_owned(),
                    PublicKey::try_from_hex_string(&r.2).unwrap(),
                )
            })
            .collect()
    }

    /// This is a 'just in case' the main code isn't keeping them in sync.
    pub async fn populate_new_people() -> Result<(), Error> {
        let sql = "INSERT or IGNORE INTO person (pubkey) SELECT DISTINCT pubkey FROM EVENT";

        task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            db.execute(sql, [])?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn generate_contact_list_event(&self) -> Result<Event, Error> {
        let mut p_tags: Vec<Tag> = Vec::new();

        let pubkeys = self.get_followed_pubkeys();
        for pubkey in &pubkeys {
            // Get their best relay
            let relays = DbPersonRelay::get_best_relays(pubkey.clone(), Direction::Write).await?;
            let maybeurl = relays.get(0);
            p_tags.push(Tag::Pubkey {
                pubkey: pubkey.clone(),
                recommended_relay_url: maybeurl.map(|(u, _)| u.to_unchecked_url()),
                petname: None,
            });
        }

        let public_key = match GLOBALS.signer.public_key() {
            Some(pk) => pk,
            None => return Err(Error::NoPrivateKey), // not even a public key
        };

        // Get the content from our latest ContactList.
        // We don't use the data, but we shouldn't clobber it.
        let content = match DbEvent::fetch_latest_contact_list(public_key.into()).await? {
            Some(c) => c.content,
            None => "".to_owned(),
        };

        // NOTICE - some clients are stuffing relay following data into the content
        // of `ContactList`s.  We don't have a set of relays that we read from, so
        // we could only do half of that even if we wanted to, and I'm not sure only
        // putting in write relays is of any use.
        let pre_event = PreEvent {
            pubkey: public_key,
            created_at: Unixtime::now().unwrap(),
            kind: EventKind::ContactList,
            tags: p_tags,
            content,
            ots: None,
        };

        GLOBALS.signer.sign_preevent(pre_event, None)
    }

    pub fn follow(&self, pubkeyhex: &PublicKeyHex, follow: bool) {
        // We can't do it now, but we spawn a task to do it soon
        let pubkeyhex = pubkeyhex.to_owned();
        tokio::spawn(async move {
            if let Err(e) = GLOBALS.people.async_follow(&pubkeyhex, follow).await {
                tracing::error!("{}", e);
            }
        });
    }

    pub async fn async_follow(&self, pubkeyhex: &PublicKeyHex, follow: bool) -> Result<(), Error> {
        // Skip if they are already followed (or already not followed)
        let already_followed = self.get_followed_pubkeys().contains(pubkeyhex);
        if follow == already_followed {
            return Ok(());
        }

        let follow: u8 = u8::from(follow);

        // Follow in database
        let sql = "INSERT INTO PERSON (pubkey, followed) values (?, ?) \
                   ON CONFLICT(pubkey) DO UPDATE SET followed=?";
        let pubkeyhex2 = pubkeyhex.to_owned();
        task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(sql)?;
            stmt.execute((pubkeyhex2.as_str(), &follow, &follow))?;
            Ok::<(), Error>(())
        })
        .await??;

        // Make sure memory matches
        if let Some(mut dbperson) = self.people.get_mut(pubkeyhex) {
            dbperson.followed = follow;
        } else {
            // load
            if let Some(person) = Self::fetch_one(pubkeyhex).await? {
                self.people.insert(pubkeyhex.to_owned(), person);
            }
        }

        if follow > 0 {
            // Add the person to the relay_picker for picking
            GLOBALS.relay_picker.add_someone(pubkeyhex.to_owned())?;
        } else {
            GLOBALS.relay_picker.remove_someone(pubkeyhex.to_owned());
        }

        Ok(())
    }

    pub async fn follow_all(
        &self,
        pubkeys: &[PublicKeyHex],
        merge: bool,
        asof: Unixtime,
    ) -> Result<(), Error> {
        // If merging, and we already follow all these keys,
        // then just bail out
        if merge {
            let mut new = false;
            let followed = self.get_followed_pubkeys();
            for pk in pubkeys {
                if !followed.contains(pk) {
                    new = true;
                    break;
                }
            }
            if !new {
                return Ok(());
            }
        }

        tracing::debug!(
            "Updating following list, {} people long, merge={}",
            pubkeys.len(),
            merge
        );

        // Make sure they are all in the database (and memory) first.
        self.create_all_if_missing(pubkeys).await?;

        // Follow in database
        let sql = format!(
            "UPDATE person SET followed=1, followed_last_updated=? \
             WHERE pubkey IN ({}) and followed_last_updated<?",
            repeat_vars(pubkeys.len())
        );

        let pubkey_strings: Vec<String> = pubkeys.iter().map(|p| p.to_string()).collect();

        task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(&sql)?;
            stmt.raw_bind_parameter(1, asof.0)?;
            let mut pos = 2;
            for pk in pubkey_strings.iter() {
                stmt.raw_bind_parameter(pos, pk)?;
                pos += 1;
            }
            stmt.raw_bind_parameter(pos, asof.0)?;
            stmt.raw_execute()?;
            Ok::<(), Error>(())
        })
        .await??;

        if !merge {
            // Unfollow in database
            let sql = format!(
                "UPDATE person SET followed=0, followed_last_updated=? \
                 WHERE pubkey NOT IN ({}) and followed_last_updated<?",
                repeat_vars(pubkeys.len())
            );

            let pubkey_strings: Vec<String> = pubkeys.iter().map(|p| p.to_string()).collect();

            task::spawn_blocking(move || {
                let maybe_db = GLOBALS.db.blocking_lock();
                let db = maybe_db.as_ref().unwrap();
                let mut stmt = db.prepare(&sql)?;
                stmt.raw_bind_parameter(1, asof.0)?;
                let mut pos = 2;
                for pk in pubkey_strings.iter() {
                    stmt.raw_bind_parameter(pos, pk)?;
                    pos += 1;
                }
                stmt.raw_bind_parameter(pos, asof.0)?;
                stmt.raw_execute()?;
                Ok::<(), Error>(())
            })
            .await??;
        }

        // Make sure memory matches
        for mut elem in self.people.iter_mut() {
            let pkh = elem.key().clone();
            let mut person = elem.value_mut();
            if person.followed_last_updated < asof.0 {
                if pubkeys.contains(&pkh) {
                    person.followed = 1;
                } else if !merge {
                    person.followed = 0;
                }
            }
        }

        // Add the people to the relay_picker for picking
        for pubkey in pubkeys.iter() {
            GLOBALS.relay_picker.add_someone(pubkey.to_owned())?;
        }

        Ok(())
    }

    pub fn mute(&self, pubkeyhex: &PublicKeyHex, mute: bool) {
        // We can't do it now, but we spawn a task to do it soon
        let pubkeyhex = pubkeyhex.to_owned();
        tokio::spawn(async move {
            if let Err(e) = GLOBALS.people.async_mute(&pubkeyhex, mute).await {
                tracing::error!("{}", e);
            }
        });
    }

    pub async fn async_mute(&self, pubkeyhex: &PublicKeyHex, mute: bool) -> Result<(), Error> {
        let mute: u8 = u8::from(mute);

        // Mute in database
        let sql = "INSERT INTO PERSON (pubkey, muted) values (?, ?) \
                   ON CONFLICT(pubkey) DO UPDATE SET muted=?";
        let pubkeyhex2 = pubkeyhex.to_owned();
        task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(sql)?;
            stmt.execute((pubkeyhex2.as_str(), &mute, &mute))?;
            Ok::<(), Error>(())
        })
        .await??;

        // Make sure memory matches
        if let Some(mut dbperson) = self.people.get_mut(pubkeyhex) {
            dbperson.muted = mute;
        } else {
            // load
            if let Some(person) = Self::fetch_one(pubkeyhex).await? {
                self.people.insert(pubkeyhex.to_owned(), person);
            }
        }

        Ok(())
    }

    // Returns true if the date passed in is newer than what we already had
    pub async fn update_relay_list_stamps(
        &self,
        pubkeyhex: PublicKeyHex,
        mut created_at: i64,
    ) -> Result<bool, Error> {
        let now = Unixtime::now().unwrap().0;

        let mut retval = false;

        if let Some(mut person) = self.people.get_mut(&pubkeyhex) {
            person.relay_list_last_received = now;
            if created_at > person.relay_list_created_at {
                retval = true;
                person.relay_list_created_at = created_at;
            } else {
                created_at = person.relay_list_created_at; // for the update below
            }
        } else {
            tracing::warn!("FIXME: RelayList for person we don't have. We should create them.");
            return Ok(false);
        }

        task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(
                "UPDATE person SET relay_list_last_received=?, \
                            relay_list_created_at=? WHERE pubkey=?",
            )?;
            stmt.execute((&now, &created_at, pubkeyhex.as_str()))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(retval)
    }

    pub async fn update_nip05_last_checked(&self, pubkeyhex: PublicKeyHex) -> Result<(), Error> {
        let now = Unixtime::now().unwrap().0;

        if let Some(mut person) = self.people.get_mut(&pubkeyhex) {
            person.nip05_last_checked = Some(now as u64);
        }

        task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare("UPDATE person SET nip05_last_checked=? WHERE pubkey=?")?;
            stmt.execute((&now, pubkeyhex.as_str()))?;
            Ok(())
        })
        .await?
    }

    pub async fn upsert_nip05_validity(
        &self,
        pubkeyhex: &PublicKeyHex,
        nip05: Option<String>,
        nip05_valid: bool,
        nip05_last_checked: u64,
    ) -> Result<(), Error> {
        // Update memory
        if let Some(mut dbperson) = self.people.get_mut(pubkeyhex) {
            if let Some(metadata) = &mut dbperson.metadata {
                metadata.nip05 = nip05.clone()
            } else {
                let mut metadata = Metadata::new();
                metadata.nip05 = nip05.clone();
                dbperson.metadata = Some(metadata);
            }
            dbperson.nip05_valid = u8::from(nip05_valid);
            dbperson.nip05_last_checked = Some(nip05_last_checked);
        }

        // Update in database
        let sql = "INSERT INTO person (pubkey, metadata, nip05_valid, nip05_last_checked) \
                   values (?, ?, ?, ?) \
                   ON CONFLICT(pubkey) DO \
                   UPDATE SET metadata=json_patch(metadata, ?), nip05_valid=?, nip05_last_checked=?";

        let pubkeyhex2 = pubkeyhex.to_owned();
        task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut metadata = Metadata::new();
            metadata.nip05 = nip05.clone();
            let metadata_json: Option<String> = Some(serde_json::to_string(&metadata)?);
            let metadata_patch = Nip05Patch { nip05 };
            let metadata_patch_str = serde_json::to_string(&metadata_patch)?;

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                pubkeyhex2.as_str(),
                &metadata_json,
                &nip05_valid,
                &nip05_last_checked,
                &metadata_patch_str,
                &nip05_valid,
                &nip05_last_checked,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    async fn fetch(criteria: Option<&str>) -> Result<Vec<DbPerson>, Error> {
        let sql = "SELECT pubkey, metadata, metadata_at, \
             nip05_valid, nip05_last_checked, \
             followed, followed_last_updated, muted, \
             relay_list_last_received, relay_list_created_at \
             FROM person"
            .to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
        };

        let output: Result<Vec<DbPerson>, Error> = task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let mut rows = stmt.query([])?;
            let mut output: Vec<DbPerson> = Vec::new();
            while let Some(row) = rows.next()? {
                let metadata_json: Option<String> = row.get(1)?;
                let metadata = match metadata_json {
                    Some(s) => serde_json::from_str(&s)?,
                    None => None,
                };
                let pk: String = row.get(0)?;
                output.push(DbPerson {
                    pubkey: PublicKeyHex::try_from_string(pk)?,
                    metadata,
                    metadata_at: row.get(2)?,
                    nip05_valid: row.get(3)?,
                    nip05_last_checked: row.get(4)?,
                    followed: row.get(5)?,
                    followed_last_updated: row.get(6)?,
                    muted: row.get(7)?,
                    relay_list_last_received: row.get(8)?,
                    relay_list_created_at: row.get(9)?,
                });
            }
            Ok(output)
        })
        .await?;

        output
    }

    async fn fetch_one(pubkeyhex: &PublicKeyHex) -> Result<Option<DbPerson>, Error> {
        let people = Self::fetch(Some(&format!("pubkey='{}'", pubkeyhex))).await?;

        if people.is_empty() {
            Ok(None)
        } else {
            Ok(Some(people[0].clone()))
        }
    }

    async fn fetch_many(pubkeys: &[&PublicKeyHex]) -> Result<Vec<DbPerson>, Error> {
        let sql = format!(
            "SELECT pubkey, metadata, metadata_at, nip05_valid, nip05_last_checked, \
             followed, followed_last_updated, muted, relay_list_last_received, \
             relay_list_created_at FROM person WHERE pubkey IN ({})",
            repeat_vars(pubkeys.len())
        );

        let pubkey_strings: Vec<String> = pubkeys.iter().map(|p| p.to_string()).collect();

        let output: Result<Vec<DbPerson>, Error> = task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;

            let mut pos = 1;
            for pk in pubkey_strings.iter() {
                stmt.raw_bind_parameter(pos, pk)?;
                pos += 1;
            }

            let mut rows = stmt.raw_query();
            let mut people: Vec<DbPerson> = Vec::new();
            while let Some(row) = rows.next()? {
                let metadata_json: Option<String> = row.get(1)?;
                let metadata = match metadata_json {
                    Some(s) => serde_json::from_str(&s)?,
                    None => None,
                };
                let pk: String = row.get(0)?;
                people.push(DbPerson {
                    pubkey: PublicKeyHex::try_from_string(pk)?,
                    metadata,
                    metadata_at: row.get(2)?,
                    nip05_valid: row.get(3)?,
                    nip05_last_checked: row.get(4)?,
                    followed: row.get(5)?,
                    followed_last_updated: row.get(6)?,
                    muted: row.get(7)?,
                    relay_list_last_received: row.get(8)?,
                    relay_list_created_at: row.get(9)?,
                });
            }

            Ok(people)
        })
        .await?;

        output
    }

    /*
        pub async fn clear_active_person(&self) {
            self.active_persons_write_relays.clear();
            *self.active_person.write().await = None;
    }
        */

    pub async fn set_active_person(&self, pubkey: PublicKeyHex) -> Result<(), Error> {
        // Set the active person
        *self.active_person.write().await = Some(pubkey.clone());

        // Load their relays
        let best_relays = DbPersonRelay::get_best_relays(pubkey, Direction::Write).await?;
        *self.active_persons_write_relays.write().await = best_relays;

        Ok(())
    }

    pub fn get_active_person(&self) -> Option<PublicKeyHex> {
        self.active_person.blocking_read().clone()
    }

    pub fn get_active_person_write_relays(&self) -> Vec<(RelayUrl, u64)> {
        self.active_persons_write_relays.blocking_read().clone()
    }

    /*
    async fn insert(person: DbPerson) -> Result<(), Error> {
        let sql = "INSERT OR IGNORE INTO person (pubkey, metadata, metadata_at, \
             nip05_valid, nip05_last_checked, followed, followed_last_updated, muted) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";

        task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let metadata_json: Option<String> = if let Some(md) = &person.metadata {
                Some(serde_json::to_string(md)?)
            } else {
                None
            };

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                &person.pubkey.0,
                &metadata_json,
                &person.metadata_at,
                &person.nip05_valid,
                &person.nip05_last_checked,
                &person.followed,
                &person.followed_last_updated,
                &person.muted,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }
     */

    /*
       pub async fn delete(criteria: &str) -> Result<(), Error> {
           let sql = format!("DELETE FROM person WHERE {}", criteria);

           task::spawn_blocking(move || {
               let maybe_db = GLOBALS.db.blocking_lock();
               let db = maybe_db.as_ref().unwrap();
               db.execute(&sql, [])?;
               Ok::<(), Error>(())
           })
           .await??;

           Ok(())
       }
    */
}

fn repeat_vars(count: usize) -> String {
    assert_ne!(count, 0);
    let mut s = "?,".repeat(count);
    // Remove trailing comma
    s.pop();
    s
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Nip05Patch {
    nip05: Option<String>,
}
