use crate::{Error, GLOBALS};
use serde::{Deserialize, Serialize};
use nostr_proto::PublicKeyHex;
use tauri::async_runtime::spawn_blocking;

#[derive(Debug, Serialize, Deserialize)]
pub struct DbPersonRelay {
    pub person: String,
    pub relay: String,
    pub recommended: u8,
    pub last_fetched: Option<u64>,
}

impl DbPersonRelay {
    #[allow(dead_code)]
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbPersonRelay>, Error> {
        let sql = "SELECT person, relay, recommended, last_fetched FROM person_relay".to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
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
        })
        .await?;

        output
    }

    /// Fetch records matching the given public keys, ordered from highest to lowest rank
    #[allow(dead_code)]
    pub async fn fetch_for_pubkeys(pubkeys: &[PublicKeyHex])
                                   -> Result<Vec<DbPersonRelay>, Error>
    {
        if pubkeys.len()==0 { return Ok(vec![]); }

        let sql = format!(
            "SELECT person, relay, recommended, person_relay.last_fetched \
             FROM person_relay \
             INNER JOIN relay ON person_relay.relay=relay.url \
             WHERE person IN ({}) ORDER BY person, relay.rank DESC",
            repeat_vars(pubkeys.len())
        );

        let pubkey_strings: Vec<String> = pubkeys.iter().map(|p| p.0.clone()).collect();

        let output: Result<Vec<DbPersonRelay>, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let rows = stmt.query_map(
                rusqlite::params_from_iter(pubkey_strings),
                |row| {
                    Ok(DbPersonRelay {
                        person: row.get(0)?,
                        relay: row.get(1)?,
                        recommended: row.get(2)?,
                        last_fetched: row.get(3)?,
                    })
                }
            )?;

            let mut output: Vec<DbPersonRelay> = Vec::new();
            for row in rows {
                output.push(row?);
            }
            Ok(output)
        })
        .await?;

        output
    }

    /// Fetch oldest last_fetched among a set of public keys for a relay
    #[allow(dead_code)]
    pub async fn fetch_oldest_last_fetched(pubkeys: &[PublicKeyHex], relay: &str)
                                           -> Result<u64, Error>
    {
        if pubkeys.len()==0 { return Ok(0); }

        let sql = format!(
            "SELECT min(last_fetched) FROM person_relay
             WHERE relay=? AND person in ({})",
            repeat_vars(pubkeys.len())
        );

        let mut params: Vec<String> = vec![relay.to_string()];
        params.extend(
            pubkeys.iter().map(|p| p.0.clone())
        );

        let output: Result<u64, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let mut rows = stmt.query_map(
                rusqlite::params_from_iter(params),
                |row| row.get(0)
            )?;
            if let Some(result) = rows.next() {
                return Ok(result?);
            } else {
                return Ok(0)
            }
        })
        .await?;

        output
    }

    #[allow(dead_code)]
    pub async fn insert(person_relay: DbPersonRelay) -> Result<(), Error> {
        let sql =
            "INSERT OR IGNORE INTO person_relay (person, relay, recommended, last_fetched) \
             VALUES (?1, ?2, ?3, ?4)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            stmt.execute((
                &person_relay.person,
                &person_relay.relay,
                &person_relay.recommended,
                &person_relay.last_fetched
            ))?;
            Ok::<(), Error>(())
        }).await??;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn update_last_fetched(relay: String, last_fetched: u64) -> Result<(), Error> {
        let sql =
            "UPDATE person_relay SET last_fetched=? where relay=?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            stmt.execute((
                &last_fetched,
                &*relay
            ))?;
            Ok::<(), Error>(())
        }).await??;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn delete(criteria: &str) -> Result<(), Error> {
        let sql = format!("DELETE FROM person_relay WHERE {}", criteria);

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            db.execute(&sql, [])?;
            Ok::<(), Error>(())
        }).await??;

        Ok(())
    }
}

fn repeat_vars(count: usize) -> String {
    assert_ne!(count, 0);
    let mut s = "?,".repeat(count);
    // Remove trailing comma
    s.pop();
    s
}
