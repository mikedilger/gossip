use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Metadata, PublicKeyHex, Unixtime};
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbPerson {
    pub pubkey: PublicKeyHex,
    pub name: Option<String>,
    pub about: Option<String>,
    pub picture: Option<String>,
    pub dns_id: Option<String>,
    pub dns_id_valid: u8,
    pub dns_id_last_checked: Option<u64>,
    pub metadata_at: Option<i64>,
    pub followed: u8,
}

impl DbPerson {
    pub fn new(pubkey: PublicKeyHex) -> DbPerson {
        DbPerson {
            pubkey,
            name: None,
            about: None,
            picture: None,
            dns_id: None,
            dns_id_valid: 0,
            dns_id_last_checked: None,
            metadata_at: None,
            followed: 0,
        }
    }

    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbPerson>, Error> {
        let sql =
            "SELECT pubkey, name, about, picture, dns_id, dns_id_valid, dns_id_last_checked, metadata_at, followed FROM person".to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
        };

        let output: Result<Vec<DbPerson>, Error> = spawn_blocking(move || {
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

    pub async fn fetch_one(pubkey: PublicKeyHex) -> Result<Option<DbPerson>, Error> {
        let people = DbPerson::fetch(Some(&format!("pubkey='{}'", pubkey))).await?;

        if people.is_empty() {
            Ok(None)
        } else {
            Ok(Some(people[0].clone()))
        }
    }

    pub async fn insert(person: DbPerson) -> Result<(), Error> {
        let sql =
            "INSERT OR IGNORE INTO person (pubkey, name, about, picture, dns_id, dns_id_valid, dns_id_last_checked, metadata_at, followed) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";

        spawn_blocking(move || {
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

    pub async fn update(person: DbPerson) -> Result<(), Error> {
        let sql =
            "UPDATE person SET name=?, about=?, picture=?, dns_id=?, dns_id_valid=?, dns_id_last_checked=?, metadata_at=?, followed=? WHERE pubkey=?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                &person.name,
                &person.about,
                &person.picture,
                &person.dns_id,
                &person.dns_id_valid,
                &person.dns_id_last_checked,
                &person.metadata_at,
                &person.followed,
                &person.pubkey.0,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn update_metadata(
        pubkey: PublicKeyHex,
        metadata: Metadata,
        created_at: Unixtime,
    ) -> Result<(), Error> {
        let sql =
            "UPDATE person SET name=?, about=?, picture=?, dns_id=?, metadata_at=? WHERE pubkey=?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                &metadata.name,
                &metadata.about,
                &metadata.picture,
                &metadata.nip05,
                &created_at.0,
                &pubkey.0,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn delete(criteria: &str) -> Result<(), Error> {
        let sql = format!("DELETE FROM person WHERE {}", criteria);

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            db.execute(&sql, [])?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn populate_new_people(follow_everybody: bool) -> Result<(), Error> {
        let sql = if follow_everybody {
            "INSERT or IGNORE INTO person (pubkey, followed) SELECT DISTINCT pubkey, 1 FROM EVENT"
        } else {
            "INSERT or IGNORE INTO person (pubkey) SELECT DISTINCT pubkey FROM EVENT"
        };

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            db.execute(sql, [])?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }
}
