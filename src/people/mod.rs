use crate::db::DbPerson;
use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Metadata, PublicKeyHex, Unixtime};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use tokio::task;

pub struct People {
    people: HashMap<PublicKeyHex, DbPerson>,
    deferred_load: HashSet<PublicKeyHex>,
    deferred_follow: HashMap<PublicKeyHex, bool>,
}

impl People {
    pub fn new() -> People {
        People {
            people: HashMap::new(),
            deferred_load: HashSet::new(),
            deferred_follow: HashMap::new(),
        }
    }

    pub async fn get_followed_pubkeys(&self) -> Vec<PublicKeyHex> {
        let mut output: Vec<PublicKeyHex> = Vec::new();
        for (_, person) in self.people.iter() {
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
        if let Some(metadata_at) = person.metadata_at {
            if asof.0 <= metadata_at {
                // Old metadata. Ignore it
                return Ok(());
            }
        }
        person.name = metadata.name;
        person.about = metadata.about;
        person.picture = metadata.picture;
        if person.dns_id != metadata.nip05 {
            person.dns_id = metadata.nip05;
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
             metadata_at, followed FROM person WHERE followed=1"
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
            // Not there. Maybe it's in the database. Defer and let syncer
            // try to load
            self.deferred_load.insert(pubkeyhex.to_owned());
            let _ = GLOBALS.to_syncer.send("sync_people".to_owned());
            None
        }
    }

    pub fn get_all(&mut self) -> Vec<DbPerson> {
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

    pub async fn sync(&mut self) -> Result<(), Error> {
        // handle deferred load
        for pubkeyhex in self.deferred_load.iter() {
            if !self.people.contains_key(pubkeyhex) {
                if let Some(person) = Self::fetch_one(pubkeyhex).await? {
                    let _ = self.people.insert(pubkeyhex.to_owned(), person);
                }
            }
        }
        self.deferred_load.clear();

        // handle deferred follow
        let df = self.deferred_follow.clone();
        for (pubkeyhex, follow) in df {
            self.async_follow(&pubkeyhex, follow).await?;
        }
        self.deferred_follow.clear();

        Ok(())
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
        self.deferred_follow
            .entry(pubkeyhex.clone())
            .and_modify(|d| *d = follow)
            .or_insert_with(|| follow);
        let _ = GLOBALS.to_syncer.send("sync_people".to_owned());
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

    pub async fn upsert_valid_nip05(
        &mut self,
        pubkeyhex: &PublicKeyHex,
        dns_id: String,
        dns_id_last_checked: u64,
    ) -> Result<(), Error> {
        // Update memory
        if let Some(dbperson) = self.people.get_mut(pubkeyhex) {
            dbperson.dns_id = Some(dns_id.clone());
            dbperson.dns_id_last_checked = Some(dns_id_last_checked);
        }

        // Update in database
        let sql = "INSERT INTO person (pubkey, dns_id, dns_id_valid, dns_id_last_checked) \
                   values (?, ?, 1, ?) \
                   ON CONFLICT(pubkey) DO UPDATE SET dns_id=?, dns_id_valid=1, dns_id_last_checked=?";

        let pubkeyhex2 = pubkeyhex.to_owned();
        task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                &pubkeyhex2.0,
                &dns_id,
                &dns_id_last_checked,
                &dns_id,
                &dns_id_last_checked,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    async fn fetch(criteria: Option<&str>) -> Result<Vec<DbPerson>, Error> {
        let sql =
            "SELECT pubkey, name, about, picture, dns_id, dns_id_valid, dns_id_last_checked, metadata_at, followed FROM person".to_owned();
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
            "INSERT OR IGNORE INTO person (pubkey, name, about, picture, dns_id, dns_id_valid, dns_id_last_checked, metadata_at, followed) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";

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
