use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Id, RelayUrl};
use tokio::task::spawn_blocking;

#[derive(Debug, Clone)]
pub struct DbRelay {
    pub url: RelayUrl,
    pub success_count: u64,
    pub failure_count: u64,
    pub rank: u64,
    pub last_connected_at: Option<u64>,
    pub last_general_eose_at: Option<u64>,
    pub read: bool,
    pub write: bool,
    pub advertise: bool,
}

impl DbRelay {
    pub fn new(url: RelayUrl) -> DbRelay {
        DbRelay {
            url,
            success_count: 0,
            failure_count: 0,
            rank: 3,
            last_connected_at: None,
            last_general_eose_at: None,
            read: false,
            write: false,
            advertise: false,
        }
    }

    pub fn attempts(&self) -> u64 {
        self.success_count + self.failure_count
    }

    pub fn success_rate(&self) -> f32 {
        let attempts = self.success_count + self.failure_count;
        if attempts == 0 {
            return 0.5;
        } // unknown, so we put it in the middle
        self.success_count as f32 / attempts as f32
    }

    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbRelay>, Error> {
        let sql = "SELECT url, success_count, failure_count, rank, last_connected_at, \
             last_general_eose_at, read, write, advertise FROM relay"
            .to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
        };

        let output: Result<Vec<DbRelay>, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let mut rows = rtry!(stmt.query([]));
            let mut output: Vec<DbRelay> = Vec::new();
            while let Some(row) = rows.next()? {
                let s: String = row.get(0)?;
                // just skip over invalid relay URLs
                if let Ok(url) = RelayUrl::try_from_str(&s) {
                    output.push(DbRelay {
                        url,
                        success_count: row.get(1)?,
                        failure_count: row.get(2)?,
                        rank: row.get(3)?,
                        last_connected_at: row.get(4)?,
                        last_general_eose_at: row.get(5)?,
                        read: row.get(6)?,
                        write: row.get(7)?,
                        advertise: row.get(8)?,
                    });
                }
            }
            Ok::<Vec<DbRelay>, Error>(output)
        })
        .await?;

        output
    }

    pub async fn fetch_one(url: &RelayUrl) -> Result<Option<DbRelay>, Error> {
        let relays = DbRelay::fetch(Some(&format!("url='{}'", url.0))).await?;

        if relays.is_empty() {
            Ok(None)
        } else {
            Ok(Some(relays[0].clone()))
        }
    }

    pub async fn insert(relay: DbRelay) -> Result<(), Error> {
        let sql = "INSERT OR IGNORE INTO relay (url, success_count, failure_count, rank, \
                   last_connected_at, last_general_eose_at, read, write, advertise) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            rtry!(stmt.execute((
                &relay.url.0,
                &relay.success_count,
                &relay.failure_count,
                &relay.rank,
                &relay.last_connected_at,
                &relay.last_general_eose_at,
                &relay.read,
                &relay.write,
                &relay.advertise,
            )));
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn update(relay: DbRelay) -> Result<(), Error> {
        let sql = "UPDATE relay SET success_count=?, failure_count=?, rank=?, \
                   last_connected_at=?, last_general_eose_at=?, read=?, write=?, advertise=? WHERE url=?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            rtry!(stmt.execute((
                &relay.success_count,
                &relay.failure_count,
                &relay.rank,
                &relay.last_connected_at,
                &relay.last_general_eose_at,
                &relay.read,
                &relay.write,
                &relay.advertise,
                &relay.url.0,
            )));
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    /// This also bumps success_count
    pub async fn update_general_eose(
        url: RelayUrl,
        last_general_eose_at: u64,
    ) -> Result<(), Error> {
        let sql =
            "UPDATE relay SET last_general_eose_at = max(?, ifnull(last_general_eose_at,0)) WHERE url = ?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            rtry!(stmt.execute((&last_general_eose_at, &url.0)));
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn clear_read_and_write() -> Result<(), Error> {
        let sql = "UPDATE relay SET read = false, write = false";
        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(sql)?;
            rtry!(stmt.execute(()));
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn update_read_and_write(
        url: RelayUrl,
        read: bool,
        write: bool,
    ) -> Result<(), Error> {
        let sql = "UPDATE relay SET read = ?, write = ?  WHERE url = ?";
        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(sql)?;
            rtry!(stmt.execute((&read, &write, &url.0)));
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn update_advertise(url: RelayUrl, advertise: bool) -> Result<(), Error> {
        let sql = "UPDATE relay SET advertise = ?  WHERE url = ?";
        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(sql)?;
            rtry!(stmt.execute((&advertise, &url.0)));
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
        let urls: Vec<RelayUrl> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(sql)?;
            let mut rows = rtry!(stmt.query([]));
            let mut maybe_urls: Vec<RelayUrl> = Vec::new();
            while let Some(row) = rows.next()? {
                let maybe_string: Option<String> = row.get(0)?;
                if let Some(string) = maybe_string {
                    if let Ok(url) = RelayUrl::try_from_str(&string) {
                        maybe_urls.push(url);
                    }
                }
            }
            Ok::<Vec<RelayUrl>, Error>(maybe_urls)
        })
        .await??;

        // FIXME this is a lot of separate sql calls
        spawn_blocking(move || {
            let sql = "INSERT OR IGNORE INTO RELAY (url, rank) VALUES (?, 3)";
            for url in urls {
                let maybe_db = GLOBALS.db.blocking_lock();
                let db = maybe_db.as_ref().unwrap();
                db.execute(sql, [&url.0])?;
            }
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn recommended_relay_for_reply(reply_to: Id) -> Result<Option<RelayUrl>, Error> {
        // Try to find a relay where the event was seen AND that I write to which
        // has a rank>1
        let sql = "SELECT url FROM relay INNER JOIN event_relay ON relay.url=event_relay.relay \
                   WHERE event_relay.event=? AND relay.write=1 AND relay.rank>1";
        let output: Option<RelayUrl> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(sql)?;
            let mut query_result = rtry!(stmt.query([reply_to.as_hex_string()]));
            if let Some(row) = query_result.next()? {
                let s: String = row.get(0)?;
                let url = RelayUrl::try_from_str(&s)?;
                Ok::<Option<RelayUrl>, Error>(Some(url))
            } else {
                Ok::<Option<RelayUrl>, Error>(None)
            }
        })
        .await??;

        if output.is_some() {
            return Ok(output);
        }

        // Fallback to finding any relay where the event was seen
        let sql = "SELECT relay FROM event_relay WHERE event=?";
        let output: Option<RelayUrl> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(sql)?;
            let mut query_result = rtry!(stmt.query([reply_to.as_hex_string()]));
            if let Some(row) = query_result.next()? {
                let s: String = row.get(0)?;
                let url = RelayUrl::try_from_str(&s)?;
                Ok::<Option<RelayUrl>, Error>(Some(url))
            } else {
                Ok::<Option<RelayUrl>, Error>(None)
            }
        })
        .await??;

        Ok(output)
    }

    /*
    pub async fn delete_relay(url: RelayUrl) -> Result<(), Error> {
        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let _ = db.execute("DELETE FROM event seen WHERE relay=?", (&url.0,));
            let _ = db.execute("UPDATE contact SET relay=null WHERE relay=?", (&url.0,));
            let _ = db.execute("DELETE FROM person_relay WHERE relay=?", (&url.0,));
            let _ = db.execute("DELETE FROM relay WHERE url=?", (&url.0,));
        })
            .await?;

        Ok(())
    }
     */

    pub async fn set_rank(url: RelayUrl, rank: u8) -> Result<(), Error> {
        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let _ = db.execute("UPDATE relay SET rank=? WHERE url=?", (&rank, &url.0));
        })
        .await?;

        Ok(())
    }
}
