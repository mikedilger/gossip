use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Id, Url};
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbRelay {
    pub dirty: bool,
    pub url: String,
    pub success_count: u64,
    pub failure_count: u64,
    pub rank: Option<u64>,
    pub last_connected_at: Option<u64>,
    pub last_general_eose_at: Option<u64>,
    pub post: bool,
}

impl DbRelay {
    pub fn new(url: String) -> Result<DbRelay, Error> {
        let u = Url::new(&url);
        if !u.is_valid_relay_url() {
            return Err(Error::InvalidUrl(u.inner().to_owned()));
        }

        Ok(DbRelay {
            dirty: false,
            url,
            success_count: 0,
            failure_count: 0,
            rank: Some(3),
            last_connected_at: None,
            last_general_eose_at: None,
            post: false,
        })
    }

    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbRelay>, Error> {
        let sql = "SELECT url, success_count, failure_count, rank, last_connected_at, \
             last_general_eose_at, post FROM relay"
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
                let postint: u32 = row.get(6)?;
                Ok(DbRelay {
                    dirty: false,
                    url: row.get(0)?,
                    success_count: row.get(1)?,
                    failure_count: row.get(2)?,
                    rank: row.get(3)?,
                    last_connected_at: row.get(4)?,
                    last_general_eose_at: row.get(5)?,
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
        let url = Url::new(&relay.url);
        if !url.is_valid_relay_url() {
            return Err(Error::InvalidUrl(relay.url.clone()));
        }

        let sql = "INSERT OR IGNORE INTO relay (url, success_count, failure_count, rank, \
                   last_connected_at, last_general_eose_at, post) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";

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
                &relay.last_connected_at,
                &relay.last_general_eose_at,
                &postint,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn update(relay: DbRelay) -> Result<(), Error> {
        let sql = "UPDATE relay SET success_count=?, failure_count=?, rank=?, \
                   last_connected_at=?, last_general_eose_at=?, post=? WHERE url=?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            let postint = i32::from(relay.post);
            stmt.execute((
                &relay.success_count,
                &relay.failure_count,
                &relay.rank,
                &relay.last_connected_at,
                &relay.last_general_eose_at,
                &postint,
                &relay.url,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    /// This also bumps success_count
    pub async fn update_success(url: String, last_connected_at: u64) -> Result<(), Error> {
        let sql = "UPDATE relay SET success_count = success_count + 1, last_connected_at = ? \
                   WHERE url = ?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((&last_connected_at, &url))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    /// This also bumps success_count
    pub async fn update_general_eose(url: String, last_general_eose_at: u64) -> Result<(), Error> {
        let sql =
            "UPDATE relay SET last_general_eose_at = max(?, ifnull(last_general_eose_at,0)) WHERE url = ?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((&last_general_eose_at, &url))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn update_post(url: String, post: bool) -> Result<(), Error> {
        let sql = "UPDATE relay SET post = ?  WHERE url = ?";
        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(sql)?;
            stmt.execute((&post, &url))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    /*
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
     */

    pub async fn populate_new_relays() -> Result<(), Error> {
        // Get relays from person_relay list
        let sql =
            "INSERT OR IGNORE INTO relay (url, rank) SELECT DISTINCT relay, 3 FROM person_relay";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            db.execute(sql, [])?;
            Ok::<(), Error>(())
        })
        .await??;

        // Select relays from 'e' and 'p' event tags
        let sql = "SELECT DISTINCT field1 FROM event_tag where (label='e' OR label='p')";
        let maybe_urls: Vec<String> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(sql)?;
            let mut rows = stmt.query([])?;
            let mut maybe_urls: Vec<String> = Vec::new();
            while let Some(row) = rows.next()? {
                let maybe_string: Option<String> = row.get(0)?;
                if let Some(string) = maybe_string {
                    maybe_urls.push(string);
                }
            }
            Ok::<Vec<String>, Error>(maybe_urls)
        })
        .await??;

        // Convert into Urls
        let urls: Vec<Url> = maybe_urls
            .iter()
            .map(|s| Url::new(s))
            .filter(|r| r.is_valid_relay_url())
            .collect();

        // FIXME this is a lot of separate sql calls
        spawn_blocking(move || {
            let sql = "INSERT OR IGNORE INTO RELAY (url, rank) VALUES (?, 3)";
            for url in urls {
                let maybe_db = GLOBALS.db.blocking_lock();
                let db = maybe_db.as_ref().unwrap();
                db.execute(sql, [&url.inner()])?;
            }
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn recommended_relay_for_reply(reply_to: Id) -> Result<Option<Url>, Error> {
        // Try to find a relay where the event was seen AND that I post to which
        // has a rank>1
        let sql = "SELECT url FROM relay INNER JOIN event_seen ON relay.url=event_seen.relay \
                   WHERE event_seen.event=? AND relay.post=1 AND relay.rank>1";
        let output: Option<Url> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(sql)?;
            let mut query_result = stmt.query([reply_to.as_hex_string()])?;
            if let Some(row) = query_result.next()? {
                let s: String = row.get(0)?;
                let url = Url::new(&s);
                Ok::<Option<Url>, Error>(Some(url))
            } else {
                Ok::<Option<Url>, Error>(None)
            }
        })
        .await??;

        if output.is_some() {
            return Ok(output);
        }

        // Fallback to finding any relay where the event was seen
        let sql = "SELECT relay FROM event_seen WHERE event=?";
        let output: Option<Url> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(sql)?;
            let mut query_result = stmt.query([reply_to.as_hex_string()])?;
            if let Some(row) = query_result.next()? {
                let s: String = row.get(0)?;
                let url = Url::new(&s);
                Ok::<Option<Url>, Error>(Some(url))
            } else {
                Ok::<Option<Url>, Error>(None)
            }
        })
        .await??;

        Ok(output)
    }
}
