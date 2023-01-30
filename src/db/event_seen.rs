use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Id, RelayUrl};
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

#[derive(Debug, Serialize, Deserialize)]
pub struct DbEventSeen {
    pub event: String,
    pub relay: String,
    pub when_seen: u64,
}

impl DbEventSeen {
    /*
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbEventSeen>, Error> {
        let sql = "SELECT event, relay, when_seen FROM event_seen".to_owned();
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
                    event: row.get(0)?,
                    relay: row.get(1)?,
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
     */

    pub async fn get_relays_for_event(id: Id) -> Result<Vec<RelayUrl>, Error> {
        let sql = "SELECT relay FROM event_seen WHERE event=?";

        let relays: Result<Vec<RelayUrl>, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(sql)?;
            stmt.raw_bind_parameter(1, id.as_hex_string())?;
            let mut rows = stmt.raw_query();
            let mut relays: Vec<RelayUrl> = Vec::new();
            while let Some(row) = rows.next()? {
                let s: String = row.get(0)?;
                // Just skip over bad relay URLs
                if let Ok(url) = RelayUrl::try_from_str(&s) {
                    relays.push(url);
                }
            }
            Ok(relays)
        })
        .await?;

        relays
    }

    pub async fn replace(event_seen: DbEventSeen) -> Result<(), Error> {
        let sql = "REPLACE INTO event_seen (event, relay, when_seen) \
             VALUES (?1, ?2, ?3)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((&event_seen.event, &event_seen.relay, &event_seen.when_seen))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    /*
        pub async fn delete(criteria: &str) -> Result<(), Error> {
            let sql = format!("DELETE FROM event_seen WHERE {}", criteria);

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
}
