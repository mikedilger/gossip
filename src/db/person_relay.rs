use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{PublicKeyHex, RelayUrl, Unixtime};
use tokio::task::spawn_blocking;

#[derive(Debug, Copy, Clone)]
pub enum Direction {
    Read,
    Write,
}

#[derive(Debug)]
pub struct DbPersonRelay {
    pub person: String,
    pub relay: RelayUrl,
    pub last_fetched: Option<u64>,
    pub last_suggested_kind3: Option<u64>,
    pub last_suggested_nip05: Option<u64>,
    pub last_suggested_bytag: Option<u64>,
    pub read: bool,
    pub write: bool,
    pub manually_paired_read: bool,
    pub manually_paired_write: bool,
}

impl DbPersonRelay {
    /// Fetch records matching the given public keys, ordered from highest to lowest rank
    pub async fn fetch_for_pubkeys(pubkeys: &[PublicKeyHex]) -> Result<Vec<DbPersonRelay>, Error> {
        if pubkeys.is_empty() {
            return Ok(vec![]);
        }

        let sql = format!(
            "SELECT person, relay, person_relay.last_fetched, \
             last_suggested_kind3, \
             last_suggested_nip05, last_suggested_bytag, \
             person_relay.read, person_relay.write, \
             person_relay.manually_paired_read, person_relay.manually_paired_write \
             FROM person_relay \
             INNER JOIN relay ON person_relay.relay=relay.url \
             WHERE person IN ({}) ORDER BY person, \
             person_relay.write DESC, \
             person_relay.manually_paired_write DESC, \
             relay.rank DESC, \
             last_suggested_kind3 DESC, \
             last_suggested_nip05 DESC, \
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
                        last_suggested_kind3: row.get(3)?,
                        last_suggested_nip05: row.get(4)?,
                        last_suggested_bytag: row.get(5)?,
                        read: row.get(6)?,
                        write: row.get(7)?,
                        manually_paired_read: row.get(8)?,
                        manually_paired_write: row.get(9)?,
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
                   last_suggested_kind3, \
                   last_suggested_nip05, last_suggested_bytag, read, write, \
                   manually_paired_read, manually_paired_write) \
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                &person_relay.person,
                &person_relay.relay.0,
                &person_relay.last_fetched,
                &person_relay.last_suggested_kind3,
                &person_relay.last_suggested_nip05,
                &person_relay.last_suggested_bytag,
                &person_relay.read,
                &person_relay.write,
                &person_relay.manually_paired_read,
                &person_relay.manually_paired_write,
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

    pub async fn set_relay_list(
        person: PublicKeyHex,
        read_relays: Vec<RelayUrl>,
        write_relays: Vec<RelayUrl>,
    ) -> Result<(), Error> {
        // Clear the current ones
        let sql1 = "UPDATE person_relay SET read=0, write=0 WHERE person=?";

        // Set the reads
        let mut sql2: String = "".to_owned();
        let mut params2: Vec<String> = vec![person.to_string()];
        if !read_relays.is_empty() {
            sql2 = format!(
                "UPDATE person_relay SET read=1 WHERE person=? AND relay IN ({})",
                repeat_vars(read_relays.len())
            );
            for relay in read_relays.iter() {
                params2.push(relay.to_string());
            }
        }

        // Set the writes
        let mut sql3: String = "".to_owned();
        let mut params3: Vec<String> = vec![person.to_string()];
        if !write_relays.is_empty() {
            sql3 = format!(
                "UPDATE person_relay SET write=1 WHERE person=? AND relay IN ({})",
                repeat_vars(write_relays.len())
            );
            for relay in write_relays.iter() {
                params3.push(relay.to_string());
            }
        }

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let inner = || -> Result<(), Error> {
                let mut stmt = db.prepare("BEGIN TRANSACTION")?;
                stmt.execute(())?;

                let mut stmt = db.prepare(sql1)?;
                stmt.execute((person.as_str(),))?;

                if !read_relays.is_empty() {
                    let mut stmt = db.prepare(&sql2)?;
                    stmt.execute(rusqlite::params_from_iter(params2))?;
                }

                if !write_relays.is_empty() {
                    let mut stmt = db.prepare(&sql3)?;
                    stmt.execute(rusqlite::params_from_iter(params3))?;
                }

                let mut stmt = db.prepare("COMMIT TRANSACTION")?;
                stmt.execute(())?;

                Ok(())
            };

            if let Err(e) = inner() {
                tracing::error!("{}", e);
                let mut stmt = db.prepare("ROLLBACK TRANSACTION")?;
                stmt.execute(())?;
            }

            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn set_manual_pairing(
        person: PublicKeyHex,
        read_relays: Vec<RelayUrl>,
        write_relays: Vec<RelayUrl>,
    ) -> Result<(), Error> {
        // Clear the current ones
        let sql1 = "UPDATE person_relay SET manually_paired_read=0, manually_paired_write=0 WHERE person=?";

        // Set the reads
        let mut sql2: String = "".to_owned();
        let mut params2: Vec<String> = vec![person.to_string()];
        if !read_relays.is_empty() {
            sql2 = format!(
                "UPDATE person_relay SET manually_paired_read=1 WHERE person=? AND relay IN ({})",
                repeat_vars(read_relays.len())
            );
            for relay in read_relays.iter() {
                params2.push(relay.to_string());
            }
        }

        // Set the writes
        let mut sql3: String = "".to_owned();
        let mut params3: Vec<String> = vec![person.to_string()];
        if !write_relays.is_empty() {
            sql3 = format!(
                "UPDATE person_relay SET manually_paired_write=1 WHERE person=? AND relay IN ({})",
                repeat_vars(write_relays.len())
            );
            for relay in write_relays.iter() {
                params3.push(relay.to_string());
            }
        }

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let inner = || -> Result<(), Error> {
                let mut stmt = db.prepare("BEGIN TRANSACTION")?;
                stmt.execute(())?;

                let mut stmt = db.prepare(sql1)?;
                stmt.execute((person.as_str(),))?;

                if !read_relays.is_empty() {
                    let mut stmt = db.prepare(&sql2)?;
                    stmt.execute(rusqlite::params_from_iter(params2))?;
                }

                if !write_relays.is_empty() {
                    let mut stmt = db.prepare(&sql3)?;
                    stmt.execute(rusqlite::params_from_iter(params3))?;
                }

                let mut stmt = db.prepare("COMMIT TRANSACTION")?;
                stmt.execute(())?;

                Ok(())
            };

            if let Err(e) = inner() {
                tracing::error!("{}", e);
                let mut stmt = db.prepare("ROLLBACK TRANSACTION")?;
                stmt.execute(())?;
            }

            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    /// This returns the relays for a person, along with a score, in order of score
    pub async fn get_best_relays(
        pubkey: PublicKeyHex,
        dir: Direction,
    ) -> Result<Vec<(RelayUrl, u64)>, Error> {
        let sql = "SELECT person, relay, last_fetched, last_suggested_kind3, \
                   last_suggested_nip05, last_suggested_bytag, read, write, \
                   manually_paired_read, manually_paired_write \
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
                        last_suggested_kind3: row.get(3)?,
                        last_suggested_nip05: row.get(4)?,
                        last_suggested_bytag: row.get(5)?,
                        read: row.get(6)?,
                        write: row.get(7)?,
                        manually_paired_read: row.get(8)?,
                        manually_paired_write: row.get(9)?,
                    };
                    dbprs.push(dbpr);
                }
            }

            match dir {
                Direction::Write => Ok(DbPersonRelay::write_rank(dbprs)),
                Direction::Read => Ok(DbPersonRelay::read_rank(dbprs)),
            }
        })
        .await?;

        let mut ranked_relays = ranked_relays?;

        let num_relays_per_person = GLOBALS.settings.read().await.num_relays_per_person as usize;

        // If we can't get enough of them, extend with some of our relays
        // at whatever the lowest score of their last one was
        if ranked_relays.len() < (num_relays_per_person + 1) {
            let how_many_more = (num_relays_per_person + 1) - ranked_relays.len();
            let last_score = if ranked_relays.is_empty() {
                20
            } else {
                ranked_relays[ranked_relays.len() - 1].1
            };
            match dir {
                Direction::Write => {
                    // substitute our read relays
                    let additional: Vec<(RelayUrl, u64)> = GLOBALS
                        .relay_tracker
                        .all_relays
                        .iter()
                        .filter_map(|r| {
                            if ranked_relays.iter().any(|(url, _)| url == r.key()) {
                                None // already in their list
                            } else if r.value().read {
                                Some((r.key().clone(), last_score))
                            } else {
                                None
                            }
                        })
                        .take(how_many_more)
                        .collect();
                    ranked_relays.extend(additional);
                }
                Direction::Read => {
                    // substitute our write relays
                    let additional: Vec<(RelayUrl, u64)> = GLOBALS
                        .relay_tracker
                        .all_relays
                        .iter()
                        .filter_map(|r| {
                            if ranked_relays.iter().any(|(url, _)| url == r.key()) {
                                None // already in their list
                            } else if r.value().write {
                                Some((r.key().clone(), last_score))
                            } else {
                                None
                            }
                        })
                        .take(how_many_more)
                        .collect();
                    ranked_relays.extend(additional);
                }
            }
        }

        Ok(ranked_relays)
    }

    // This ranks the relays that a person writes to
    pub fn write_rank(mut dbprs: Vec<DbPersonRelay>) -> Vec<(RelayUrl, u64)> {
        // This is the ranking we are using. There might be reasons
        // for ranking differently.
        //   write (score=20)    [ they claim (to us) ]
        //   manually_paired_write (score=20)    [ we claim (to us) ]
        //   kind3 tag (score=5) [ we say ]
        //   nip05 (score=4)     [ they claim, unsigned ]
        //   fetched (score=3)   [ we found ]
        //   bytag (score=1)     [ someone else mentions ]

        let now = Unixtime::now().unwrap().0 as u64;
        let mut output: Vec<(RelayUrl, u64)> = Vec::new();

        let scorefn = |when: u64, fade_period: u64, base: u64| -> u64 {
            let dur = now.saturating_sub(when); // seconds since
            let periods = (dur / fade_period) + 1; // minimum one period
            base / periods
        };

        for dbpr in dbprs.drain(..) {
            let mut score = 0;

            // 'write' is an author-signed explicit claim of where they write
            if dbpr.write || dbpr.manually_paired_write {
                score += 20;
            }

            // kind3 is our memory of where we are following someone
            if let Some(when) = dbpr.last_suggested_kind3 {
                score += scorefn(when, 60 * 60 * 24 * 30, 7);
            }

            // nip05 is an unsigned dns-based author claim of using this relay
            if let Some(when) = dbpr.last_suggested_nip05 {
                score += scorefn(when, 60 * 60 * 24 * 15, 4);
            }

            // last_fetched is gossip verified happened-to-work-before
            if let Some(when) = dbpr.last_fetched {
                score += scorefn(when, 60 * 60 * 24 * 3, 3);
            }

            // last_suggested_bytag is an anybody-signed suggestion
            if let Some(when) = dbpr.last_suggested_bytag {
                score += scorefn(when, 60 * 60 * 24 * 2, 1);
            }

            // Prune score=0 associations
            if score == 0 {
                continue;
            }

            output.push((dbpr.relay, score));
        }

        output.sort_by(|(_, score1), (_, score2)| score2.cmp(score1));

        // prune everything below a score of 20, but only after the first 6 entries
        while output.len() > 6 && output[output.len() - 1].1 < 20 {
            let _ = output.pop();
        }

        output
    }

    // This ranks the relays that a person reads from
    pub fn read_rank(mut dbprs: Vec<DbPersonRelay>) -> Vec<(RelayUrl, u64)> {
        // This is the ranking we are using. There might be reasons
        // for ranking differently.
        //   read (score=20)    [ they claim (to us) ]
        //   manually_paired_read (score=20)    [ we claim (to us) ]
        //   kind3 tag (score=5) [ we say ]
        //   nip05 (score=4)     [ they claim, unsigned ]
        //   fetched (score=3)   [ we found ]
        //   bytag (score=1)     [ someone else mentions ]

        let now = Unixtime::now().unwrap().0 as u64;
        let mut output: Vec<(RelayUrl, u64)> = Vec::new();

        let scorefn = |when: u64, fade_period: u64, base: u64| -> u64 {
            let dur = now.saturating_sub(when); // seconds since
            let periods = (dur / fade_period) + 1; // minimum one period
            base / periods
        };

        for dbpr in dbprs.drain(..) {
            let mut score = 0;

            // 'read' is an author-signed explicit claim of where they read
            if dbpr.read || dbpr.manually_paired_read {
                score += 20;
            }

            // kind3 is our memory of where we are following someone
            if let Some(when) = dbpr.last_suggested_kind3 {
                score += scorefn(when, 60 * 60 * 24 * 30, 7);
            }

            // nip05 is an unsigned dns-based author claim of using this relay
            if let Some(when) = dbpr.last_suggested_nip05 {
                score += scorefn(when, 60 * 60 * 24 * 15, 4);
            }

            // last_fetched is gossip verified happened-to-work-before
            if let Some(when) = dbpr.last_fetched {
                score += scorefn(when, 60 * 60 * 24 * 3, 3);
            }

            // last_suggested_bytag is an anybody-signed suggestion
            if let Some(when) = dbpr.last_suggested_bytag {
                score += scorefn(when, 60 * 60 * 24 * 2, 1);
            }

            // Prune score=0 associations
            if score == 0 {
                continue;
            }

            output.push((dbpr.relay, score));
        }

        output.sort_by(|(_, score1), (_, score2)| score2.cmp(score1));

        // prune everything below a score 20, but only after the first 6 entries
        while output.len() > 6 && output[output.len() - 1].1 < 20 {
            let _ = output.pop();
        }
        output
    }
}

fn repeat_vars(count: usize) -> String {
    assert_ne!(count, 0);
    let mut s = "?,".repeat(count);
    // Remove trailing comma
    s.pop();
    s
}
