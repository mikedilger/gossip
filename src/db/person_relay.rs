use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::PublicKeyHex;
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DbPersonRelay {
    pub person: String,
    pub relay: String,
    pub last_fetched: Option<u64>,
    pub last_suggested_kind2: Option<u64>,
    pub last_suggested_kind3: Option<u64>,
    pub last_suggested_nip23: Option<u64>,
    pub last_suggested_nip35: Option<u64>,
    pub last_suggested_bytag: Option<u64>,
}

impl DbPersonRelay {
    /*
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbPersonRelay>, Error> {
        let sql = "SELECT person, relay, last_fetched, last_suggested_kind2, last_suggested_kind3, last_suggested_nip23, last_suggested_nip35, last_suggested_bytag FROM person_relay".to_owned();
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
                    last_fetched: row.get(2)?,
                    last_suggested_kind2: row.get(3)?,
                    last_suggested_kind3: row.get(4)?,
                    last_suggested_nip23: row.get(5)?,
                    last_suggested_nip35: row.get(6)?,
                    last_suggested_bytag: row.get(7)?,
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
     */

    /// Fetch records matching the given public keys, ordered from highest to lowest rank
    pub async fn fetch_for_pubkeys(pubkeys: &[PublicKeyHex]) -> Result<Vec<DbPersonRelay>, Error> {
        if pubkeys.is_empty() {
            return Ok(vec![]);
        }

        let sql = format!(
            "SELECT person, relay, person_relay.last_fetched, \
             last_suggested_kind2, last_suggested_kind3, last_suggested_nip23, \
             last_suggested_nip35, last_suggested_bytag \
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
            let rows = stmt.query_map(rusqlite::params_from_iter(pubkey_strings), |row| {
                Ok(DbPersonRelay {
                    person: row.get(0)?,
                    relay: row.get(1)?,
                    last_fetched: row.get(2)?,
                    last_suggested_kind2: row.get(3)?,
                    last_suggested_kind3: row.get(4)?,
                    last_suggested_nip23: row.get(5)?,
                    last_suggested_nip35: row.get(6)?,
                    last_suggested_bytag: row.get(7)?,
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

    /*
    /// Fetch oldest last_fetched among a set of public keys for a relay
    pub async fn fetch_oldest_last_fetched(
        pubkeys: &[PublicKeyHex],
        relay: &str,
    ) -> Result<u64, Error> {
        if pubkeys.is_empty() {
            return Ok(0);
        }

        let sql = format!(
            "SELECT min(coalesce(last_fetched,0)) FROM person_relay
             WHERE relay=? AND person in ({})",
            repeat_vars(pubkeys.len())
        );

        let mut params: Vec<String> = vec![relay.to_string()];
        params.extend(pubkeys.iter().map(|p| p.0.clone()));

        let output: Result<u64, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let mut rows = stmt.query_map(rusqlite::params_from_iter(params), |row| row.get(0))?;
            if let Some(result) = rows.next() {
                Ok(result?)
            } else {
                Ok(0)
            }
        })
        .await?;

        output
    }
     */

    pub async fn insert(person_relay: DbPersonRelay) -> Result<(), Error> {
        let sql = "INSERT OR IGNORE INTO person_relay (person, relay, last_fetched, \
                   last_suggested_kind2, last_suggested_kind3, last_suggested_nip23, \
                   last_suggested_nip35, last_suggested_bytag) \
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                &person_relay.person,
                &person_relay.relay,
                &person_relay.last_fetched,
                &person_relay.last_suggested_kind2,
                &person_relay.last_suggested_kind3,
                &person_relay.last_suggested_nip23,
                &person_relay.last_suggested_nip35,
                &person_relay.last_suggested_bytag,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn upsert_last_fetched(
        person: String,
        relay: String,
        last_fetched: u64,
    ) -> Result<(), Error> {
        let sql = "INSERT INTO person_relay (person, relay, last_fetched) \
                   VALUES (?, ?, ?) \
                   ON CONFLICT(person, relay) DO UPDATE SET last_fetched=?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((&person, &relay, &last_fetched, &last_fetched))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn upsert_last_suggested_bytag(
        person: String,
        relay: String,
        last_suggested_bytag: u64,
    ) -> Result<(), Error> {
        let sql = "INSERT INTO person_relay (person, relay, last_suggested_bytag) \
                   VALUES (?, ?, ?) \
                   ON CONFLICT(person, relay) DO UPDATE SET last_suggested_bytag=?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                &person,
                &relay,
                &last_suggested_bytag,
                &last_suggested_bytag,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn upsert_last_suggested_nip35(
        person: PublicKeyHex,
        relay: String,
        last_suggested_nip35: u64,
    ) -> Result<(), Error> {
        let sql = "INSERT INTO person_relay (person, relay, last_suggested_nip35) \
                   VALUES (?, ?, ?) \
                   ON CONFLICT(person, relay) DO UPDATE SET last_suggested_nip35=?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                &person.0,
                &relay,
                &last_suggested_nip35,
                &last_suggested_nip35,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn fetch_matching(
        people: Vec<PublicKeyHex>,
        relays: Vec<String>,
    ) -> Result<Vec<DbPersonRelay>, Error> {
        if people.is_empty() {
            return Ok(vec![]);
        }
        if relays.is_empty() {
            return Ok(vec![]);
        }

        let sql = format!(
            "SELECT \
             person, relay, person_relay.last_fetched, \
             last_suggested_kind2, last_suggested_kind3, last_suggested_nip23, \
             last_suggested_nip35, last_suggested_bytag \
             FROM person_relay
             INNER JOIN relay ON person_relay.relay=relay.url \
             WHERE person IN ({}) and relay in ({}) \
             ORDER BY person, relay.rank DESC",
            repeat_vars(people.len()),
            repeat_vars(relays.len())
        );

        let pubkey_strings: Vec<String> = people.iter().map(|p| p.0.clone()).collect();

        let output: Result<Vec<DbPersonRelay>, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut params: Vec<String> = pubkey_strings;
            params.extend(relays);

            let mut stmt = db.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
                Ok(DbPersonRelay {
                    person: row.get(0)?,
                    relay: row.get(1)?,
                    last_fetched: row.get(2)?,
                    last_suggested_kind2: row.get(3)?,
                    last_suggested_kind3: row.get(4)?,
                    last_suggested_nip23: row.get(5)?,
                    last_suggested_nip35: row.get(6)?,
                    last_suggested_bytag: row.get(7)?,
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

    /*
        pub async fn delete(criteria: &str) -> Result<(), Error> {
            let sql = format!("DELETE FROM person_relay WHERE {}", criteria);

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

fn repeat_vars(count: usize) -> String {
    assert_ne!(count, 0);
    let mut s = "?,".repeat(count);
    // Remove trailing comma
    s.pop();
    s
}
