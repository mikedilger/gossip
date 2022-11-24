use crate::{Error, GLOBALS};
use nostr_proto::PublicKey;
use serde::{Deserialize, Serialize};
use tauri::async_runtime::spawn_blocking;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbPerson {
    pub public_key: String,
    pub name: Option<String>,
    pub about: Option<String>,
    pub picture: Option<String>,
    pub dns_id: Option<String>,
    pub dns_id_valid: u8,
    pub dns_id_last_checked: Option<u64>,
    pub followed: u8,
}

impl DbPerson {
    #[allow(dead_code)]
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbPerson>, Error> {
        let sql =
            "SELECT public_key, name, about, picture, dns_id, dns_id_valid, dns_id_last_checked, followed FROM person".to_owned();
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
                    public_key: row.get(0)?,
                    name: row.get(1)?,
                    about: row.get(2)?,
                    picture: row.get(3)?,
                    dns_id: row.get(4)?,
                    dns_id_valid: row.get(5)?,
                    dns_id_last_checked: row.get(6)?,
                    followed: row.get(7)?,
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

    #[allow(dead_code)]
    pub async fn fetch_one(pubkey: PublicKey) -> Result<Option<DbPerson>, Error> {
        let people = DbPerson::fetch(
            Some(&format!("public_key='{}'",pubkey.as_hex_string()))
        ).await?;

        if people.len() == 0 {
            Ok(None)
        } else {
            Ok(Some(people[0].clone()))
        }
    }

    #[allow(dead_code)]
    pub async fn insert(person: DbPerson) -> Result<(), Error> {
        let sql =
            "INSERT OR IGNORE INTO person (public_key, name, about, picture, dns_id, dns_id_valid, dns_id_last_checked, followed) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            stmt.execute((
                &person.public_key,
                &person.name,
                &person.about,
                &person.picture,
                &person.dns_id,
                &person.dns_id_valid,
                &person.dns_id_last_checked,
                &person.followed
            ))?;
            Ok::<(), Error>(())
        }).await??;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn update(person: DbPerson) -> Result<(), Error> {
        let sql =
            "UPDATE person SET name=?, about=?, picture=?, dns_id=?, dns_id_valid=?, dns_id_last_checked=?, followed=? WHERE public_key=?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            stmt.execute((
                &person.name,
                &person.about,
                &person.picture,
                &person.dns_id,
                &person.dns_id_valid,
                &person.dns_id_last_checked,
                &person.followed,
                &person.public_key
            ))?;
            Ok::<(), Error>(())
        }).await??;

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
        }).await??;

        Ok(())
    }
}
