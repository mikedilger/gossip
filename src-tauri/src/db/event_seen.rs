use crate::{Error, GLOBALS};
use serde::{Deserialize, Serialize};
use tauri::async_runtime::spawn_blocking;

#[derive(Debug, Serialize, Deserialize)]
pub struct DbEventSeen {
    pub id: String,
    pub url: String,
    pub when_seen: String
}

impl DbEventSeen {
    #[allow(dead_code)]
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbEventSeen>, Error> {
        let sql =
            "SELECT id, url, when_seen FROM event_seen".to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
        };

        let output: Result<Vec<DbEventSeen>, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                Ok(DbEventSeen {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    when_seen: row.get(2)?,
                })
            })?;

            let mut output: Vec<DbEventSeen> = Vec::new();
            for row in rows {
                output.push(row?);
            }
            Ok(output)
        })
        .await?;

        output
    }

    #[allow(dead_code)]
    pub async fn insert(event_seen: DbEventSeen) -> Result<(), Error> {
        let sql =
            "INSERT OR IGNORE INTO event_seen (id, url, when_seen) \
             VALUES (?1, ?2, ?3)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            stmt.execute((
                &event_seen.id,
                &event_seen.url,
                &event_seen.when_seen
            ))?;
            Ok::<(), Error>(())
        }).await??;

        Ok(())
    }
}
