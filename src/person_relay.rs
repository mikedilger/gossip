use crate::error::Error;
use crate::globals::GLOBALS;
use crate::relay::Relay;
use gossip_relay_picker::Direction;
use nostr_types::{PublicKey, PublicKeyHex, RelayUrl, Unixtime};
use speedy::{Readable, Writable};
use tokio::task::spawn_blocking;

#[derive(Debug, Readable, Writable)]
pub struct PersonRelay {
    // The person
    pub pubkey: PublicKey,

    // The relay associated with that person
    pub url: RelayUrl,

    // The last time we fetched one of the person's events from this relay
    pub last_fetched: Option<u64>,

    // When we follow someone at a relay
    pub last_suggested_kind3: Option<u64>,

    // When we get their nip05 and it specifies this relay
    pub last_suggested_nip05: Option<u64>,

    // Updated when a 'p' tag on any event associates this person and relay via the
    // recommended_relay_url field
    pub last_suggested_bytag: Option<u64>,

    pub read: bool,

    pub write: bool,

    // When we follow someone at a relay, this is set true
    pub manually_paired_read: bool,

    // When we follow someone at a relay, this is set true
    pub manually_paired_write: bool,
}

impl PersonRelay {
    pub async fn insert(person_relay: PersonRelay) -> Result<(), Error> {
        let sql = "INSERT OR IGNORE INTO person_relay (person, relay, last_fetched, \
                   last_suggested_kind3, \
                   last_suggested_nip05, last_suggested_bytag, read, write, \
                   manually_paired_read, manually_paired_write) \
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

        let pubkeyhex: PublicKeyHex = person_relay.pubkey.into();

        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(sql)?;
            rtry!(stmt.execute((
                pubkeyhex.as_str(),
                &person_relay.url.0,
                &person_relay.last_fetched,
                &person_relay.last_suggested_kind3,
                &person_relay.last_suggested_nip05,
                &person_relay.last_suggested_bytag,
                &person_relay.read,
                &person_relay.write,
                &person_relay.manually_paired_read,
                &person_relay.manually_paired_write,
            )));
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn upsert_last_fetched(
        pubkey: PublicKey,
        url: RelayUrl,
        last_fetched: u64,
    ) -> Result<(), Error> {
        let sql = "INSERT INTO person_relay (person, relay, last_fetched) \
                   VALUES (?, ?, ?) \
                   ON CONFLICT(person, relay) DO UPDATE SET last_fetched=?";

        let pubkeyhex: PublicKeyHex = pubkey.into();
        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(sql)?;
            rtry!(stmt.execute((pubkeyhex.as_str(), &url.0, &last_fetched, &last_fetched)));
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn upsert_last_suggested_kind3(
        pubkey: PublicKey,
        url: RelayUrl,
        last_suggested_kind3: u64,
    ) -> Result<(), Error> {
        let sql = "INSERT INTO person_relay (person, relay, last_suggested_kind3) \
                   VALUES (?, ?, ?) \
                   ON CONFLICT(person, relay) DO UPDATE SET last_suggested_kind3=?";

        let pubkeyhex: PublicKeyHex = pubkey.into();
        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(sql)?;
            rtry!(stmt.execute((
                pubkeyhex.as_str(),
                &url.0,
                &last_suggested_kind3,
                &last_suggested_kind3,
            )));
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn upsert_last_suggested_bytag(
        pubkey: PublicKey,
        url: RelayUrl,
        last_suggested_bytag: u64,
    ) -> Result<(), Error> {
        let sql = "INSERT INTO person_relay (person, relay, last_suggested_bytag) \
                   VALUES (?, ?, ?) \
                   ON CONFLICT(person, relay) DO UPDATE SET last_suggested_bytag=?";

        let pubkeyhex: PublicKeyHex = pubkey.into();
        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(sql)?;
            rtry!(stmt.execute((
                pubkeyhex.as_str(),
                &url.0,
                &last_suggested_bytag,
                &last_suggested_bytag,
            )));
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn upsert_last_suggested_nip05(
        pubkey: PublicKey,
        url: RelayUrl,
        last_suggested_nip05: u64,
    ) -> Result<(), Error> {
        let sql = "INSERT INTO person_relay (person, relay, last_suggested_nip05) \
                   VALUES (?, ?, ?) \
                   ON CONFLICT(person, relay) DO UPDATE SET last_suggested_nip05=?";

        let pubkeyhex: PublicKeyHex = pubkey.into();
        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(sql)?;
            rtry!(stmt.execute((
                pubkeyhex.as_str(),
                &url.0,
                &last_suggested_nip05,
                &last_suggested_nip05,
            )));
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn set_relay_list(
        pubkey: PublicKey,
        read_relays: Vec<RelayUrl>,
        write_relays: Vec<RelayUrl>,
    ) -> Result<(), Error> {
        let pubkeyhex: PublicKeyHex = pubkey.into();

        // Clear the current ones
        let sql1 = "UPDATE person_relay SET read=0, write=0 WHERE person=?";

        // Set the reads
        let mut sql2: String = "".to_owned();
        let mut params2: Vec<String> = vec![pubkeyhex.to_string()];
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
        let mut params3: Vec<String> = vec![pubkeyhex.to_string()];
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
            let db = GLOBALS.db.blocking_lock();
            let inner = || -> Result<(), Error> {
                let mut stmt = db.prepare("BEGIN TRANSACTION")?;
                rtry!(stmt.execute(()));

                let mut stmt = db.prepare(sql1)?;
                rtry!(stmt.execute((pubkeyhex.as_str(),)));

                if !read_relays.is_empty() {
                    let mut stmt = db.prepare(&sql2)?;
                    rtry!(stmt.execute(rusqlite::params_from_iter(params2)));
                }

                if !write_relays.is_empty() {
                    let mut stmt = db.prepare(&sql3)?;
                    rtry!(stmt.execute(rusqlite::params_from_iter(params3)));
                }

                let mut stmt = db.prepare("COMMIT TRANSACTION")?;
                rtry!(stmt.execute(()));

                Ok(())
            };

            if let Err(e) = inner() {
                tracing::error!("{}", e);
                let mut stmt = db.prepare("ROLLBACK TRANSACTION")?;
                rtry!(stmt.execute(()));
            }

            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn set_manual_pairing(
        pubkey: PublicKey,
        read_relays: Vec<RelayUrl>,
        write_relays: Vec<RelayUrl>,
    ) -> Result<(), Error> {
        let pubkeyhex: PublicKeyHex = pubkey.into();

        // Clear the current ones
        let sql1 = "UPDATE person_relay SET manually_paired_read=0, manually_paired_write=0 WHERE person=?";

        // Set the reads
        let mut sql2: String = "".to_owned();
        let mut params2: Vec<String> = vec![pubkeyhex.to_string()];
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
        let mut params3: Vec<String> = vec![pubkeyhex.to_string()];
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
            let db = GLOBALS.db.blocking_lock();
            let inner = || -> Result<(), Error> {
                let mut stmt = db.prepare("BEGIN TRANSACTION")?;
                rtry!(stmt.execute(()));

                let mut stmt = db.prepare(sql1)?;
                rtry!(stmt.execute((pubkeyhex.as_str(),)));

                if !read_relays.is_empty() {
                    let mut stmt = db.prepare(&sql2)?;
                    rtry!(stmt.execute(rusqlite::params_from_iter(params2)));
                }

                if !write_relays.is_empty() {
                    let mut stmt = db.prepare(&sql3)?;
                    rtry!(stmt.execute(rusqlite::params_from_iter(params3)));
                }

                let mut stmt = db.prepare("COMMIT TRANSACTION")?;
                rtry!(stmt.execute(()));

                Ok(())
            };

            if let Err(e) = inner() {
                tracing::error!("{}", e);
                let mut stmt = db.prepare("ROLLBACK TRANSACTION")?;
                rtry!(stmt.execute(()));
            }

            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    /// This returns the relays for a person, along with a score, in order of score
    pub async fn get_best_relays(
        pubkey: PublicKey,
        dir: Direction,
    ) -> Result<Vec<(RelayUrl, u64)>, Error> {
        let sql = "SELECT person, relay, last_fetched, last_suggested_kind3, \
                   last_suggested_nip05, last_suggested_bytag, read, write, \
                   manually_paired_read, manually_paired_write \
                   FROM person_relay WHERE person=?";

        let pubkeyhex: PublicKeyHex = pubkey.into();
        let ranked_relays: Result<Vec<(RelayUrl, u64)>, Error> = spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(sql)?;
            stmt.raw_bind_parameter(1, pubkeyhex.as_str())?;
            let mut rows = stmt.raw_query();

            let mut dbprs: Vec<PersonRelay> = Vec::new();
            while let Some(row) = rows.next()? {
                let pk: String = row.get(0)?;
                let s: String = row.get(1)?;
                if let Ok(pubkey) = PublicKey::try_from_hex_string(&pk) {
                    if let Ok(url) = RelayUrl::try_from_str(&s) {
                        let dbpr = PersonRelay {
                            pubkey,
                            url,
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
            }

            match dir {
                Direction::Write => Ok(PersonRelay::write_rank(dbprs)),
                Direction::Read => Ok(PersonRelay::read_rank(dbprs)),
            }
        })
        .await?;

        let mut ranked_relays = ranked_relays?;

        let num_relays_per_person = GLOBALS.settings.read().num_relays_per_person as usize;

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
                        .storage
                        .filter_relays(|r| {
                            // not already in their list
                            !ranked_relays.iter().any(|(url, _)| *url == r.url)
                                && r.has_usage_bits(Relay::READ)
                        })?
                        .iter()
                        .map(|r| (r.url.clone(), last_score))
                        .take(how_many_more)
                        .collect();

                    ranked_relays.extend(additional);
                }
                Direction::Read => {
                    // substitute our write relays???
                    let additional: Vec<(RelayUrl, u64)> = GLOBALS
                        .storage
                        .filter_relays(|r| {
                            // not already in their list
                            !ranked_relays.iter().any(|(url, _)| *url == r.url)
                                && r.has_usage_bits(Relay::WRITE)
                        })?
                        .iter()
                        .map(|r| (r.url.clone(), last_score))
                        .take(how_many_more)
                        .collect();

                    ranked_relays.extend(additional);
                }
            }
        }

        Ok(ranked_relays)
    }

    // This ranks the relays that a person writes to
    pub fn write_rank(mut dbprs: Vec<PersonRelay>) -> Vec<(RelayUrl, u64)> {
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

            output.push((dbpr.url, score));
        }

        output.sort_by(|(_, score1), (_, score2)| score2.cmp(score1));

        // prune everything below a score of 20, but only after the first 6 entries
        while output.len() > 6 && output[output.len() - 1].1 < 20 {
            let _ = output.pop();
        }

        output
    }

    // This ranks the relays that a person reads from
    pub fn read_rank(mut dbprs: Vec<PersonRelay>) -> Vec<(RelayUrl, u64)> {
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

            output.push((dbpr.url, score));
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
