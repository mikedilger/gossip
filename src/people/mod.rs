use crate::db::DbPerson;
use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Metadata, PublicKey, PublicKeyHex, Unixtime};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use tokio::task;

pub struct People {
    people: HashMap<PublicKey, DbPerson>,
    deferred_load: HashSet<PublicKey>,
}

impl People {
    pub fn new() -> People {
        People {
            people: HashMap::new(),
            deferred_load: HashSet::new(),
        }
    }

    pub async fn get_followed_pubkeys(&self) -> Vec<PublicKey> {
        let mut output: Vec<PublicKey> = Vec::new();
        for (_, person) in self.people.iter() {
            if let Ok(pubkey) = PublicKey::try_from_hex_string(&person.pubkey.0) {
                output.push(pubkey);
            }
        }
        output
    }

    pub async fn create_if_missing(&mut self, pubkey: PublicKey) -> Result<(), Error> {
        if self.people.contains_key(&pubkey) {
            return Ok(());
        }

        // Try loading from the database
        let maybe_dbperson = DbPerson::fetch_one(pubkey.into()).await?;
        if let Some(dbperson) = maybe_dbperson {
            // Insert into the map
            self.people.insert(pubkey, dbperson);
        } else {
            // Create new
            let dbperson = DbPerson {
                pubkey: pubkey.into(),
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
            self.people.insert(pubkey, dbperson.clone());
            // Insert into the database
            DbPerson::insert(dbperson).await?;
        }

        Ok(())
    }

    pub async fn update_metadata(
        &mut self,
        pubkey: PublicKey,
        metadata: Metadata,
        asof: Unixtime,
    ) -> Result<(), Error> {
        // Sync in from database first
        self.create_if_missing(pubkey).await?;

        // Update the map
        let person = self.people.get_mut(&pubkey).unwrap();
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
        let pubkeyhex: PublicKeyHex = person.pubkey.clone();
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
                &pubkeyhex.0,
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
            if let Ok(pubkey) = PublicKey::try_from_hex_string(&person.pubkey.0) {
                self.people.insert(pubkey, person);
            }
        }

        Ok(())
    }

    pub fn get(&mut self, pubkey: PublicKey) -> Option<DbPerson> {
        if self.people.contains_key(&pubkey) {
            self.people.get(&pubkey).cloned()
        } else {
            // Not there. Maybe it's in the database. Defer and let syncer
            // try to load
            self.deferred_load.insert(pubkey);
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
        for pubkey in self.deferred_load.iter() {
            if !self.people.contains_key(pubkey) {
                if let Some(person) = DbPerson::fetch_one((*pubkey).into()).await? {
                    let _ = self.people.insert(*pubkey, person);
                }
            }
        }
        self.deferred_load.clear();
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
}
