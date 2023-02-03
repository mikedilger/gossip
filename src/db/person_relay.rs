use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{PublicKeyHex, RelayUrl, Unixtime};
use tokio::task::spawn_blocking;

#[derive(Debug)]
pub struct DbPersonRelay {
    pub person: String,
    pub relay: RelayUrl,
    pub last_fetched: Option<u64>,
    pub last_suggested_kind2: Option<u64>,
    pub last_suggested_kind3: Option<u64>,
    pub last_suggested_nip23: Option<u64>,
    pub last_suggested_nip05: Option<u64>,
    pub last_suggested_bytag: Option<u64>,
}

impl DbPersonRelay {
    /// Fetch records matching the given public keys, ordered from highest to lowest rank
    pub async fn fetch_for_pubkeys(pubkeys: &[PublicKeyHex]) -> Result<Vec<DbPersonRelay>, Error> {
        if pubkeys.is_empty() {
            return Ok(vec![]);
        }

        let sql = format!(
            "SELECT person, relay, person_relay.last_fetched, \
             last_suggested_kind2, last_suggested_kind3, last_suggested_nip23, \
             last_suggested_nip05, last_suggested_bytag \
             FROM person_relay \
             INNER JOIN relay ON person_relay.relay=relay.url \
             WHERE person IN ({}) ORDER BY person, relay.rank DESC, \
             last_suggested_nip23 DESC, last_suggested_kind3 DESC, \
             last_suggested_nip05 DESC, last_suggested_kind2 DESC, \
             last_fetched DESC, last_suggested_bytag DESC",
            repeat_vars(pubkeys.len())
        );

        let pubkey_strings: Vec<String> = pubkeys.iter().map(|p| p.to_string()).collect();

        let output: Result<Vec<DbPersonRelay>, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let mut rows = stmt.query(rusqlite::params_from_iter(pubkey_strings))?;
            let mut output: Vec<DbPersonRelay> = Vec::new();
            while let Some(row) = rows.next()? {
                let s: String = row.get(1)?;
                // Just skip over bad relay URLs
                if let Ok(url) = RelayUrl::try_from_str(&s) {
                    output.push(DbPersonRelay {
                        person: row.get(0)?,
                        relay: url,
                        last_fetched: row.get(2)?,
                        last_suggested_kind2: row.get(3)?,
                        last_suggested_kind3: row.get(4)?,
                        last_suggested_nip23: row.get(5)?,
                        last_suggested_nip05: row.get(6)?,
                        last_suggested_bytag: row.get(7)?,
                    });
                }
            }

            Ok(output)
        })
        .await?;

        output
    }

    pub async fn insert(person_relay: DbPersonRelay) -> Result<(), Error> {
        let sql = "INSERT OR IGNORE INTO person_relay (person, relay, last_fetched, \
                   last_suggested_kind2, last_suggested_kind3, last_suggested_nip23, \
                   last_suggested_nip05, last_suggested_bytag) \
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                &person_relay.person,
                &person_relay.relay.0,
                &person_relay.last_fetched,
                &person_relay.last_suggested_kind2,
                &person_relay.last_suggested_kind3,
                &person_relay.last_suggested_nip23,
                &person_relay.last_suggested_nip05,
                &person_relay.last_suggested_bytag,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn upsert_last_fetched(
        person: String,
        relay: RelayUrl,
        last_fetched: u64,
    ) -> Result<(), Error> {
        let sql = "INSERT INTO person_relay (person, relay, last_fetched) \
                   VALUES (?, ?, ?) \
                   ON CONFLICT(person, relay) DO UPDATE SET last_fetched=?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((&person, &relay.0, &last_fetched, &last_fetched))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn upsert_last_suggested_kind3(
        person: String,
        relay: RelayUrl,
        last_suggested_kind3: u64,
    ) -> Result<(), Error> {
        let sql = "INSERT INTO person_relay (person, relay, last_suggested_kind3) \
                   VALUES (?, ?, ?) \
                   ON CONFLICT(person, relay) DO UPDATE SET last_suggested_kind3=?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                &person,
                &relay.0,
                &last_suggested_kind3,
                &last_suggested_kind3,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }
    pub async fn upsert_last_suggested_bytag(
        person: String,
        relay: RelayUrl,
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
                &relay.0,
                &last_suggested_bytag,
                &last_suggested_bytag,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn upsert_last_suggested_nip05(
        person: PublicKeyHex,
        relay: RelayUrl,
        last_suggested_nip05: u64,
    ) -> Result<(), Error> {
        let sql = "INSERT INTO person_relay (person, relay, last_suggested_nip05) \
                   VALUES (?, ?, ?) \
                   ON CONFLICT(person, relay) DO UPDATE SET last_suggested_nip05=?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                person.as_str(),
                &relay.0,
                &last_suggested_nip05,
                &last_suggested_nip05,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn upsert_last_suggested_nip23(
        person: PublicKeyHex,
        relay: RelayUrl,
        last_suggested_nip23: u64,
    ) -> Result<(), Error> {
        let sql = "INSERT INTO person_relay (person, relay, last_suggested_nip23) \
                   VALUES (?, ?, ?) \
                   ON CONFLICT(person, relay) DO UPDATE SET last_suggested_nip23=?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                person.as_str(),
                &relay.0,
                &last_suggested_nip23,
                &last_suggested_nip23,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    /// This returns the best relays for the person along with a score, in order of score
    pub async fn get_best_relays(pubkey: PublicKeyHex) -> Result<Vec<(RelayUrl, u64)>, Error> {
        let sql = "SELECT person, relay, last_suggested_nip23, last_suggested_kind3, \
                   last_suggested_nip05, last_fetched, last_suggested_kind2, \
                   last_suggested_bytag \
                   FROM person_relay WHERE person=?";

        let ranked_relays: Result<Vec<(RelayUrl, u64)>, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(sql)?;
            stmt.raw_bind_parameter(1, pubkey.as_str())?;
            let mut rows = stmt.raw_query();

            let mut dbprs: Vec<DbPersonRelay> = Vec::new();
            while let Some(row) = rows.next()? {
                let s: String = row.get(1)?;
                // Just skip over bad relay URLs
                if let Ok(url) = RelayUrl::try_from_str(&s) {
                    let dbpr = DbPersonRelay {
                        person: row.get(0)?,
                        relay: url,
                        last_fetched: row.get(2)?,
                        last_suggested_kind2: row.get(3)?,
                        last_suggested_kind3: row.get(4)?,
                        last_suggested_nip23: row.get(5)?,
                        last_suggested_nip05: row.get(6)?,
                        last_suggested_bytag: row.get(7)?,
                    };
                    dbprs.push(dbpr);
                }
            }

            Ok(DbPersonRelay::rank(dbprs))
        })
        .await?;

        ranked_relays
    }

    // This ranks the relays
    pub fn rank(mut dbprs: Vec<DbPersonRelay>) -> Vec<(RelayUrl, u64)> {
        // This is the ranking we are using. There might be reasons
        // for ranking differently:
        // nip23 (score=10) > kind3 (score=8) > nip05 (score=6) > fetched (score=4)
        //   > kind2 (score=2) > bytag (score=1)

        let now = Unixtime::now().unwrap().0 as u64;
        let mut output: Vec<(RelayUrl, u64)> = Vec::new();

        let scorefn = |when: u64, fade_period: u64, base: u64| -> u64 {
            let dur = now.saturating_sub(when); // seconds since
            let periods = (dur / fade_period) + 1; // minimum one period
            base / periods
        };

        for dbpr in dbprs.drain(..) {
            let mut score = 0;
            // nip23 is an author-signed explicit claim of using this relay
            if let Some(when) = dbpr.last_suggested_nip23 {
                score += scorefn(when, 60 * 60 * 24 * 30, 15);
            }
            // kind3 is a temporary (not NIPped) author-signed explicit claim of using this relay
            if let Some(when) = dbpr.last_suggested_kind3 {
                score += scorefn(when, 60 * 60 * 24 * 30, 15);
            }
            // kind2 is an author-signed recommended relay list
            if let Some(when) = dbpr.last_suggested_kind2 {
                score += scorefn(when, 60 * 60 * 24 * 30, 10);
            }
            // nip05 is an unsigned dns claim of using this relay
            if let Some(when) = dbpr.last_suggested_nip05 {
                score += scorefn(when, 60 * 60 * 24 * 15, 6);
            }
            // last_fetched is gossip verified happened-to-work-before
            if let Some(when) = dbpr.last_fetched {
                score += scorefn(when, 60 * 60 * 24 * 3, 6);
            }
            // last_suggested_bytag is an anybody-signed suggestion
            if let Some(when) = dbpr.last_suggested_bytag {
                score += scorefn(when, 60 * 60 * 24 * 2, 1);
            }
            output.push((dbpr.relay, score));
        }

        output.sort_by(|(_, score1), (_, score2)| score2.cmp(score1));

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
