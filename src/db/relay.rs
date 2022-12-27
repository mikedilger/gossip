use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::Url;
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbRelay {
    pub dirty: bool,
    pub url: String,
    pub success_count: u64,
    pub failure_count: u64,
    pub rank: Option<u64>,
    pub last_success_at: Option<u64>,
    pub post: bool,
}

impl DbRelay {
    pub fn new(url: String) -> Result<DbRelay, Error> {
        let _ = Url::new_validated(&url)?;

        Ok(DbRelay {
            dirty: false,
            url,
            success_count: 0,
            failure_count: 0,
            rank: Some(3),
            last_success_at: None,
            post: false,
        })
    }

    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbRelay>, Error> {
        let sql =
            "SELECT url, success_count, failure_count, rank, last_success_at, post FROM relay"
                .to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
        };

        let output: Result<Vec<DbRelay>, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                let postint: u32 = row.get(5)?;
                Ok(DbRelay {
                    dirty: false,
                    url: row.get(0)?,
                    success_count: row.get(1)?,
                    failure_count: row.get(2)?,
                    rank: row.get(3)?,
                    last_success_at: row.get(4)?,
                    post: (postint > 0),
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

    pub async fn fetch_one(url: &Url) -> Result<Option<DbRelay>, Error> {
        let relays = DbRelay::fetch(Some(&format!("url='{}'", url))).await?;

        if relays.is_empty() {
            Ok(None)
        } else {
            Ok(Some(relays[0].clone()))
        }
    }

    pub async fn insert(relay: DbRelay) -> Result<(), Error> {
        let _ = Url::new_validated(&relay.url)?;

        let sql = "INSERT OR IGNORE INTO relay (url, success_count, failure_count, rank, last_success_at, post) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            let postint = i32::from(relay.post);
            stmt.execute((
                &relay.url,
                &relay.success_count,
                &relay.failure_count,
                &relay.rank,
                &relay.last_success_at,
                &postint,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn update(relay: DbRelay) -> Result<(), Error> {
        let sql = "UPDATE relay SET success_count=?, failure_count=?, rank=?, last_success_at=?, post=? WHERE url=?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            let postint = i32::from(relay.post);
            stmt.execute((
                &relay.success_count,
                &relay.failure_count,
                &relay.rank,
                &relay.last_success_at,
                &postint,
                &relay.url,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    /// This also bumps success_count
    pub async fn update_success(url: String, last_success_at: u64) -> Result<(), Error> {
        let sql = "UPDATE relay SET success_count = success_count + 1, last_success_at = ? \
                   WHERE url = ?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((&last_success_at, &url))?;
            Ok::<(), Error>(())
        })
        .await??;

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
        })
        .await??;

        Ok(())
    }

    pub async fn populate_new_relays() -> Result<(), Error> {
        // Get from person_relay list
        let sql =
            "INSERT OR IGNORE INTO relay (url, rank) SELECT DISTINCT relay, 3 FROM person_relay";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            db.execute(sql, [])?;
            Ok::<(), Error>(())
        })
        .await??;

        // Get from 'e' and 'p' tags
        let sql =
            "INSERT OR IGNORE INTO RELAY (url, rank) SELECT DISTINCT field1, 3 FROM event_tag where (label='e' OR label='p') and field1 like 'ws%://%'";

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
