use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Id, RelayUrl};
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

#[derive(Debug, Serialize, Deserialize)]
pub struct DbEventRelay {
    pub event: String,
    pub relay: String,
    pub when_seen: u64,
}

impl DbEventRelay {
    /*
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbEventRelay>, Error> {
        let sql = "SELECT event, relay, when_seen FROM event_relay".to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
        };

        let output: Result<Vec<DbEventRelay>, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                Ok(DbEventRelay {
                    event: row.get(0)?,
                    relay: row.get(1)?,
                    when_seen: row.get(2)?,
                })
            })?;

            let mut output: Vec<DbEventRelay> = Vec::new();
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
        let sql = "SELECT relay FROM event_relay WHERE event=?";

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

    pub async fn replace(event_relay: DbEventRelay) -> Result<(), Error> {
        let sql = "REPLACE INTO event_relay (event, relay, when_seen) \
             VALUES (?1, ?2, ?3)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                &event_relay.event,
                &event_relay.relay,
                &event_relay.when_seen,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    /*
        pub async fn delete(criteria: &str) -> Result<(), Error> {
            let sql = format!("DELETE FROM event_relay WHERE {}", criteria);

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
