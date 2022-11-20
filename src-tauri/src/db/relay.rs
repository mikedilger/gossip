use crate::{Error, GLOBALS};
use serde::{Deserialize, Serialize};
use tauri::async_runtime::spawn_blocking;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbRelay {
    pub url: String,
    pub last_up: Option<u64>,
    pub last_try: Option<u64>,
    pub last_fetched: Option<u64>,
    pub rank: Option<u64>,
}

impl DbRelay {
    #[allow(dead_code)]
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbRelay>, Error> {
        let sql = "SELECT url, last_up, last_try, last_fetched, rank FROM relay".to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
        };

        let output: Result<Vec<DbRelay>, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                Ok(DbRelay {
                    url: row.get(0)?,
                    last_up: row.get(1)?,
                    last_try: row.get(2)?,
                    last_fetched: row.get(3)?,
                    rank: row.get(4)?,
                })
            })?;

            let mut output: Vec<DbRelay> = Vec::new();
            for row in rows {
                output.push(row?);
            }
            Ok(output)
        })
        .await?;

        output
    }

    #[allow(dead_code)]
    pub async fn insert(relay: DbRelay) -> Result<(), Error> {
        let sql =
            "INSERT OR IGNORE INTO relay (url, last_up, last_try, last_fetched, rank) \
             VALUES (?1, ?2, ?3, ?4, ?5)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            stmt.execute((
                &relay.url,
                &relay.last_up,
                &relay.last_try,
                &relay.last_fetched,
                &relay.rank
            ))?;
            Ok::<(), Error>(())
        }).await??;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn delete(criteria: &str) -> Result<(), Error> {
        let sql = format!("DELETE FROM relay WHERE {}", criteria);

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            db.execute(&sql, [])?;
            Ok::<(), Error>(())
        }).await??;

        Ok(())
    }
}
