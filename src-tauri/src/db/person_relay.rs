use crate::{Error, GLOBALS};
use serde::{Serialize, Deserialize};
use tauri::async_runtime::spawn_blocking;

#[derive(Debug, Serialize, Deserialize)]
pub struct DbPersonRelay {
    pub person: String,
    pub relay: String,
    pub recommended: u8,
    pub last_fetched: Option<String>,
}

impl DbPersonRelay {
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbPersonRelay>, Error> {

        let sql = "SELECT person, relay, recommended, last_fetched FROM person_relay".to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit)
        };

        let output: Result<Vec<DbPersonRelay>, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                Ok(DbPersonRelay {
                    person: row.get(0)?,
                    relay: row.get(1)?,
                    recommended: row.get(2)?,
                    last_fetched: row.get(3)?,
                })
            })?;

            let mut output: Vec<DbPersonRelay> = Vec::new();
            for row in rows {
                output.push(row?);
            }
            Ok(output)
        }).await?;

        output
    }
}
