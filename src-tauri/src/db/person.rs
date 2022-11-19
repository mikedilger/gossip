use crate::{Error, GLOBALS};
use serde::{Deserialize, Serialize};
use tauri::async_runtime::spawn_blocking;

#[derive(Debug, Serialize, Deserialize)]
pub struct DbPerson {
    pub public_key: String,
    pub name: Option<String>,
    pub about: Option<String>,
    pub picture: Option<String>,
    pub nip05: Option<String>,
    pub followed: u8,
}

impl DbPerson {
    #[allow(dead_code)]
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbPerson>, Error> {
        let sql =
            "SELECT public_key, name, about, picture, nip05, followed FROM person".to_owned();
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
                    nip05: row.get(4)?,
                    followed: row.get(5)?,
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
    pub async fn insert(person: DbPerson) -> Result<(), Error> {
        let sql =
            "INSERT OR IGNORE INTO person (public_key, name, about, picture, nip05, followed) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            stmt.execute((
                &person.public_key,
                &person.name,
                &person.about,
                &person.picture,
                &person.nip05,
                &person.followed
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
