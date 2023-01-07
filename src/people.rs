use crate::db::DbPerson;
use crate::error::Error;
use crate::globals::GLOBALS;
use image::RgbaImage;
use nostr_types::{Metadata, PublicKey, PublicKeyHex, Unixtime, Url};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::task;

pub struct People {
    people: HashMap<PublicKeyHex, DbPerson>,

    // We fetch (with Fetcher), process, and temporarily hold avatars
    // until the UI next asks for them, at which point we remove them
    // and hand them over. This way we can do the work that takes
    // longer and the UI can do as little work as possible.
    avatars_temp: HashMap<PublicKeyHex, RgbaImage>,
    avatars_pending_processing: HashSet<PublicKeyHex>,
    avatars_failed: HashSet<PublicKeyHex>,
}

impl People {
    pub fn new() -> People {
        People {
            people: HashMap::new(),
            avatars_temp: HashMap::new(),
            avatars_pending_processing: HashSet::new(),
            avatars_failed: HashSet::new(),
        }
    }

    pub fn get_followed_pubkeys(&self) -> Vec<PublicKeyHex> {
        let mut output: Vec<PublicKeyHex> = Vec::new();
        for person in self
            .people
            .iter()
            .filter_map(|(_, p)| if p.followed == 1 { Some(p) } else { None })
        {
            output.push(person.pubkey.clone());
        }
        output
    }

    pub async fn create_if_missing(&mut self, pubkeyhex: &PublicKeyHex) -> Result<(), Error> {
        if self.people.contains_key(pubkeyhex) {
            return Ok(());
        }

        // Try loading from the database
        let maybe_dbperson = Self::fetch_one(pubkeyhex).await?;

        if let Some(dbperson) = maybe_dbperson {
            // Insert into the map
            self.people.insert(pubkeyhex.to_owned(), dbperson);
        } else {
            // Create new
            let dbperson = DbPerson {
                pubkey: pubkeyhex.to_owned(),
                name: None,
                about: None,
                picture: None,
                dns_id: None,
                dns_id_valid: 0,
                dns_id_last_checked: None,
                metadata_at: None,
                followed: 0,
                followed_last_updated: 0,
            };
            // Insert into the map
            self.people.insert(pubkeyhex.to_owned(), dbperson.clone());
            // Insert into the database
            Self::insert(dbperson).await?;
        }

        Ok(())
    }

    pub async fn update_metadata(
        &mut self,
        pubkeyhex: &PublicKeyHex,
        metadata: Metadata,
        asof: Unixtime,
    ) -> Result<(), Error> {
        // Sync in from database first
        self.create_if_missing(pubkeyhex).await?;

        // Update the map
        let person = self.people.get_mut(pubkeyhex).unwrap();

        // Determine whether to update it
        let mut doit = person.metadata_at.is_none();
        if let Some(metadata_at) = person.metadata_at {
            if asof.0 > metadata_at {
                doit = true;
            }
        }
        if doit {
            // Process fresh metadata

            person.name = metadata.get("name");
            person.about = metadata.get("about");
            person.picture = metadata.get("picture");
            if person.dns_id != metadata.get("nip05") {
                person.dns_id = metadata.get("nip05");
                person.dns_id_valid = 0; // changed, so reset to invalid
                person.dns_id_last_checked = None; // we haven't checked this one yet
            }
            person.metadata_at = Some(asof.0);

            // Update the database
            let person = person.clone();
            let pubkeyhex2 = pubkeyhex.to_owned();
            task::spawn_blocking(move || {
                let maybe_db = GLOBALS.db.blocking_lock();
                let db = maybe_db.as_ref().unwrap();

                let mut stmt = db.prepare(
                    "UPDATE person SET name=?, about=?, picture=?, dns_id=?, metadata_at=? WHERE pubkey=?"
                )?;
                stmt.execute((
                    &person.name,
                    &person.about,
                    &person.picture,
                    &person.dns_id,
                    &person.metadata_at,
                    &pubkeyhex2.0,
                ))?;
                Ok::<(), Error>(())
            })
                .await??;
        }

        // Remove from failed avatars list so the UI will try to fetch the avatar again
        GLOBALS.failed_avatars.write().await.remove(pubkeyhex);

        let person = person.to_owned();

        // Recheck nip05 every day if invalid, and every two weeks if valid
        // FIXME make these settings
        let recheck_duration = if person.dns_id_valid > 0 {
            Duration::from_secs(60 * 60 * 24 * 14)
        } else {
            Duration::from_secs(60 * 60 * 24)
        };

        // Maybe validate nip05
        if let Some(last) = person.dns_id_last_checked {
            if Unixtime::now().unwrap() - Unixtime(last as i64) > recheck_duration {
                // recheck
                self.update_dns_id_last_checked(person.pubkey.clone())
                    .await?;
                task::spawn(async move {
                    if let Err(e) = crate::nip05::validate_nip05(person).await {
                        tracing::error!("{}", e);
                    }
                });
            }
        } else {
            self.update_dns_id_last_checked(person.pubkey.clone())
                .await?;
            task::spawn(async move {
                if let Err(e) = crate::nip05::validate_nip05(person).await {
                    tracing::error!("{}", e);
                }
            });
        }

        Ok(())
    }

    pub async fn load_all_followed(&mut self) -> Result<(), Error> {
        if !self.people.is_empty() {
            return Err(Error::Internal(
                "load_all_followed should only be called before people is otherwise used."
                    .to_owned(),
            ));
        }

        let sql =
            "SELECT pubkey, name, about, picture, dns_id, dns_id_valid, dns_id_last_checked, \
             metadata_at, followed, followed_last_updated FROM person WHERE followed=1"
                .to_owned();

        let output: Result<Vec<DbPerson>, Error> = task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                Ok(DbPerson {
                    pubkey: PublicKeyHex(row.get(0)?),
                    name: row.get(1)?,
                    about: row.get(2)?,
                    picture: row.get(3)?,
                    dns_id: row.get(4)?,
                    dns_id_valid: row.get(5)?,
                    dns_id_last_checked: row.get(6)?,
                    metadata_at: row.get(7)?,
                    followed: row.get(8)?,
                    followed_last_updated: row.get(9)?,
                })
            })?;
            let mut output: Vec<DbPerson> = Vec::new();
            for row in rows {
                output.push(row?);
            }
            Ok(output)
        })
        .await?;

        for person in output? {
            self.people.insert(person.pubkey.clone(), person);
        }

        Ok(())
    }

    pub fn get(&mut self, pubkeyhex: &PublicKeyHex) -> Option<DbPerson> {
        if self.people.contains_key(pubkeyhex) {
            self.people.get(pubkeyhex).cloned()
        } else {
            // We can't get it now, but we can setup a task to do it soon
            let pubkeyhex = pubkeyhex.to_owned();
            tokio::spawn(async move {
                let mut people = GLOBALS.people.write().await;
                #[allow(clippy::map_entry)]
                if !people.people.contains_key(&pubkeyhex) {
                    match People::fetch_one(&pubkeyhex).await {
                        Ok(Some(person)) => {
                            let _ = people.people.insert(pubkeyhex, person);
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
        let mut v: Vec<DbPerson> = self.people.values().map(|p| p.to_owned()).collect();
        v.sort_by(|a, b| {
            let c = a.name.cmp(&b.name);
            if c == Ordering::Equal {
                a.pubkey.cmp(&b.pubkey)
            } else {
                c
            }
        });
        v
    }

    // If returns Err, means you're never going to get it so stop trying.
    pub fn get_avatar(&mut self, pubkeyhex: &PublicKeyHex) -> Result<Option<image::RgbaImage>, ()> {
        // If we have it, hand it over (we won't need a copy anymore)
        if let Some(th) = self.avatars_temp.remove(pubkeyhex) {
            return Ok(Some(th));
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
        if person.picture.is_none() {
            return Err(());
        }

        // FIXME: we could get metadata that sets this while we are running, so just failing for
        //        the duration of the client isn't quite right. But for now, retrying is taxing.
        let url = Url::new(person.picture.as_ref().unwrap());
        if !url.is_valid() {
            return Err(());
        }

        match GLOBALS.fetcher.try_get(url) {
            Ok(None) => Ok(None),
            Ok(Some(bytes)) => {
                // Finish this later
                let apubkeyhex = pubkeyhex.to_owned();
                tokio::spawn(async move {
                    let image = match image::load_from_memory(&bytes) {
                        // DynamicImage
                        Ok(di) => di,
                        Err(_) => {
                            let _ = GLOBALS
                                .people
                                .write()
                                .await
                                .avatars_failed
                                .insert(apubkeyhex.clone());
                            return;
                        }
                    };
                    let image = image.resize(
                        crate::AVATAR_SIZE,
                        crate::AVATAR_SIZE,
                        image::imageops::FilterType::Nearest,
                    ); // DynamicImage
                    let image_buffer = image.into_rgba8(); // RgbaImage (ImageBuffer)

                    GLOBALS
                        .people
                        .write()
                        .await
                        .avatars_temp
                        .insert(apubkeyhex, image_buffer);
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
    pub fn get_ids_from_prefix(&self, mut prefix: &str) -> Vec<(String, PublicKey)> {
        // work with or without the @ symbol:
        if prefix.starts_with('@') {
            prefix = &prefix[1..]
        }
        self.people
            .iter()
            .filter_map(|(_, person)| {
                if let Some(name) = &person.name {
                    if name.starts_with(prefix) {
                        let pubkey = PublicKey::try_from_hex_string(&person.pubkey).unwrap(); // FIXME
                        return Some((name.clone(), pubkey));
                    }
                }
                None
            })
            .take(10)
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

    pub fn follow(&mut self, pubkeyhex: &PublicKeyHex, follow: bool) {
        // We can't do it now, but we spawn a task to do it soon
        let pubkeyhex = pubkeyhex.to_owned();
        tokio::spawn(async move {
            let mut people = GLOBALS.people.write().await;
            if let Err(e) = people.async_follow(&pubkeyhex, follow).await {
                tracing::error!("{}", e);
            }
        });
    }

    pub async fn async_follow(
        &mut self,
        pubkeyhex: &PublicKeyHex,
        follow: bool,
    ) -> Result<(), Error> {
        let f: u8 = u8::from(follow);

        // Follow in database
        let sql = "INSERT INTO PERSON (pubkey, followed) values (?, ?) \
                   ON CONFLICT(pubkey) DO UPDATE SET followed=?";
        let pubkeyhex2 = pubkeyhex.to_owned();
        task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(sql)?;
            stmt.execute((&pubkeyhex2.0, &f, &f))?;
            Ok::<(), Error>(())
        })
        .await??;

        // Make sure memory matches
        if let Some(dbperson) = self.people.get_mut(pubkeyhex) {
            dbperson.followed = f;
        } else {
            // load
            if let Some(person) = Self::fetch_one(pubkeyhex).await? {
                self.people.insert(pubkeyhex.to_owned(), person);
            }
        }

        Ok(())
    }

    pub async fn update_dns_id_last_checked(
        &mut self,
        pubkeyhex: PublicKeyHex,
    ) -> Result<(), Error> {
        task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare("UPDATE person SET dns_id_last_checked=? WHERE pubkey=?")?;
            let now = Unixtime::now().unwrap().0;
            stmt.execute((&now, &pubkeyhex.0))?;
            Ok::<(), Error>(())
        })
        .await??;
        Ok(())
    }

    pub async fn upsert_nip05_validity(
        &mut self,
        pubkeyhex: &PublicKeyHex,
        dns_id: Option<String>,
        dns_id_valid: bool,
        dns_id_last_checked: u64,
    ) -> Result<(), Error> {
        // Update memory
        if let Some(dbperson) = self.people.get_mut(pubkeyhex) {
            dbperson.dns_id = dns_id.clone();
            dbperson.dns_id_valid = u8::from(dns_id_valid);
            dbperson.dns_id_last_checked = Some(dns_id_last_checked);
        }

        // Update in database
        let sql = "INSERT INTO person (pubkey, dns_id, dns_id_valid, dns_id_last_checked) \
                   values (?, ?, ?, ?) \
                   ON CONFLICT(pubkey) DO UPDATE SET dns_id=?, dns_id_valid=?, dns_id_last_checked=?";

        let pubkeyhex2 = pubkeyhex.to_owned();
        task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                &pubkeyhex2.0,
                &dns_id,
                &dns_id_valid,
                &dns_id_last_checked,
                &dns_id,
                &dns_id_valid,
                &dns_id_last_checked,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    async fn fetch(criteria: Option<&str>) -> Result<Vec<DbPerson>, Error> {
        let sql =
            "SELECT pubkey, name, about, picture, dns_id, dns_id_valid, dns_id_last_checked, \
             metadata_at, followed, followed_last_updated FROM person"
                .to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
        };

        let output: Result<Vec<DbPerson>, Error> = task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                Ok(DbPerson {
                    pubkey: PublicKeyHex(row.get(0)?),
                    name: row.get(1)?,
                    about: row.get(2)?,
                    picture: row.get(3)?,
                    dns_id: row.get(4)?,
                    dns_id_valid: row.get(5)?,
                    dns_id_last_checked: row.get(6)?,
                    metadata_at: row.get(7)?,
                    followed: row.get(8)?,
                    followed_last_updated: row.get(9)?,
                })
            })?;

            let mut output: Vec<DbPerson> = Vec::new();
            for row in rows {
                output.push(row?);
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

    async fn insert(person: DbPerson) -> Result<(), Error> {
        let sql =
            "INSERT OR IGNORE INTO person (pubkey, name, about, picture, dns_id, dns_id_valid, \
             dns_id_last_checked, metadata_at, followed, followed_last_updated) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)";

        task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                &person.pubkey.0,
                &person.name,
                &person.about,
                &person.picture,
                &person.dns_id,
                &person.dns_id_valid,
                &person.dns_id_last_checked,
                &person.metadata_at,
                &person.followed,
                &person.followed_last_updated,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

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
