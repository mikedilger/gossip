use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Event, EventKind, IdHex, PublicKeyHex};
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DbEvent {
    pub id: IdHex,
    pub raw: String,
    pub pubkey: PublicKeyHex,
    pub created_at: i64,
    pub kind: u64,
    pub content: String,
    pub ots: Option<String>,
}

impl DbEvent {
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbEvent>, Error> {
        let sql = "SELECT id, raw, pubkey, created_at, kind, content, ots FROM event".to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
        };

        let output: Result<Vec<DbEvent>, Error> = spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(&sql)?;
            let mut rows = stmt.query([])?;
            let mut output: Vec<DbEvent> = Vec::new();
            while let Some(row) = rows.next()? {
                let id: String = row.get(0)?;
                let pk: String = row.get(2)?;
                output.push(DbEvent {
                    id: IdHex::try_from_string(id)?,
                    raw: row.get(1)?,
                    pubkey: PublicKeyHex::try_from_string(pk)?,
                    created_at: row.get(3)?,
                    kind: row.get(4)?,
                    content: row.get(5)?,
                    ots: row.get(6)?,
                })
            }
            Ok(output)
        })
        .await?;

        output
    }

    pub async fn fetch_latest_contact_list(
        pubkeyhex: PublicKeyHex,
    ) -> Result<Option<Event>, Error> {
        let sql = "SELECT raw FROM event WHERE event.kind=3 AND event.pubkey=? ORDER BY created_at DESC LIMIT 1";

        let output: Result<Vec<Event>, Error> = spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(sql)?;
            stmt.raw_bind_parameter(1, pubkeyhex.as_str())?;
            let mut rows = stmt.raw_query();
            let mut events: Vec<Event> = Vec::new();
            while let Some(row) = rows.next()? {
                let raw: String = row.get(0)?;
                let event: Event = serde_json::from_str(&raw)?;
                events.push(event);
            }
            Ok(events)
        })
        .await?;

        Ok(output?.drain(..).next())
    }

    pub async fn fetch_relay_lists() -> Result<Vec<Event>, Error> {
        // FIXME, only get the last per pubkey
        let sql = "SELECT raw FROM event WHERE event.kind=10002";

        let output: Result<Vec<Event>, Error> = spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(sql)?;
            let mut rows = stmt.raw_query();
            let mut events: Vec<Event> = Vec::new();
            while let Some(row) = rows.next()? {
                let raw: String = row.get(0)?;
                let event: Event = serde_json::from_str(&raw)?;
                events.push(event);
            }
            Ok(events)
        })
        .await?;

        output
    }

    pub async fn fetch_reply_related(since: i64) -> Result<Vec<DbEvent>, Error> {
        let public_key: PublicKeyHex = match GLOBALS.signer.public_key() {
            None => return Ok(vec![]),
            Some(pk) => pk.into(),
        };

        let kinds: String = GLOBALS
            .settings
            .read()
            .feed_related_event_kinds()
            .iter()
            .map(|e| <EventKind as Into<u64>>::into(*e))
            .map(|e| e.to_string())
            .collect::<Vec<String>>()
            .join(",");

        let sql = format!(
            "SELECT id, raw, pubkey, created_at, kind, content, ots FROM event \
             LEFT JOIN event_tag ON event.id=event_tag.event \
             WHERE event.kind IN ({}) \
             AND event_tag.label='p' AND event_tag.field0=? \
             AND created_at > ? \
             ORDER BY created_at ASC",
            kinds
        );

        let output: Result<Vec<DbEvent>, Error> = spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(&sql)?;
            stmt.raw_bind_parameter(1, public_key.as_str())?;
            stmt.raw_bind_parameter(2, since)?;
            let mut rows = stmt.raw_query();
            let mut events: Vec<DbEvent> = Vec::new();
            while let Some(row) = rows.next()? {
                let id: String = row.get(0)?;
                let pk: String = row.get(2)?;
                let event = DbEvent {
                    id: IdHex::try_from_str(&id)?,
                    raw: row.get(1)?,
                    pubkey: PublicKeyHex::try_from_str(&pk)?,
                    created_at: row.get(3)?,
                    kind: row.get(4)?,
                    content: row.get(5)?,
                    ots: row.get(6)?,
                };
                events.push(event);
            }
            Ok(events)
        })
        .await?;

        output
    }

    /*
    pub async fn fetch_by_ids(ids: Vec<IdHex>) -> Result<Vec<DbEvent>, Error> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let sql = format!(
            "SELECT id, raw, pubkey, created_at, kind, content, ots FROM event WHERE id IN ({})",
            repeat_vars(ids.len())
        );

        let output: Result<Vec<DbEvent>, Error> = spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let id_strings: Vec<String> = ids.iter().map(|p| p.0.clone()).collect();

            let mut stmt = db.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(id_strings), |row| {
                Ok(DbEvent {
                    id: IdHex(row.get(0)?),
                    raw: row.get(1)?,
                    pubkey: PublicKeyHex(row.get(2)?),
                    created_at: row.get(3)?,
                    kind: row.get(4)?,
                    content: row.get(5)?,
                    ots: row.get(6)?,
                })
            })?;

            let mut output: Vec<DbEvent> = Vec::new();
            for row in rows {
                output.push(row?);
            }
            Ok(output)
        })
        .await?;

        output
    }
     */

    pub async fn insert(event: DbEvent) -> Result<(), Error> {
        let sql = "INSERT OR IGNORE INTO event (id, raw, pubkey, created_at, kind, content, ots) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";

        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                event.id.as_str(),
                &event.raw,
                event.pubkey.as_str(),
                &event.created_at,
                &event.kind,
                &event.content,
                &event.ots,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    // This is for replaceable (not parameterized!) events only.
    // Returns true if it inserted something, false if it didn't have to.
    pub async fn replace(event: DbEvent) -> Result<bool, Error> {
        // Delete anything older
        let sql = "DELETE FROM event WHERE pubkey=? and kind=? and created_at<?".to_owned();
        let pubkey: String = event.pubkey.as_str().to_owned();
        let kind: u64 = event.kind;
        let created_at: u64 = event.created_at as u64;
        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            db.execute(&sql, (&pubkey, &kind, &created_at))?;
            Ok::<(), Error>(())
        })
        .await??;

        // Check if anything remains (which must then be newer)
        let sql = "SELECT count(*) FROM event WHERE pubkey=? and kind=?".to_owned();
        let pubkey: String = event.pubkey.as_str().to_owned();
        let kind: u64 = event.kind;
        let count: usize = spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(&sql)?;
            stmt.raw_bind_parameter(1, &pubkey)?;
            stmt.raw_bind_parameter(2, kind)?;
            let mut rows = stmt.raw_query();
            if let Some(row) = rows.next()? {
                return Ok(row.get(0)?);
            }
            Ok::<usize, Error>(0)
        })
        .await??;

        // If nothing is newer, save this event
        if count == 0 {
            Self::insert(event).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // Returns true if it inserted something, false if it didn't have to.
    pub async fn replace_parameterized(event: DbEvent, parameter: String) -> Result<bool, Error> {
        // Delete anything older
        let sql = "DELETE FROM event WHERE pubkey=? and kind=? and created_at<? and id IN (SELECT event FROM event_tag WHERE event=? and label='d' AND field0=?)".to_owned();
        let pubkey: String = event.pubkey.as_str().to_owned();
        let kind: u64 = event.kind;
        let created_at: u64 = event.created_at as u64;
        let id: String = event.id.as_str().to_owned();
        let param: String = parameter.clone();
        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            db.execute(&sql, (&pubkey, &kind, &created_at, &id, &param))?;
            Ok::<(), Error>(())
        })
        .await??;

        // Check if anything remains (which must then be newer)
        let sql = "SELECT count(*) FROM event WHERE pubkey=? and kind=? AND id IN (SELECT event FROM event_tag WHERE event=? AND label='d' AND field0=?)".to_owned();
        let pubkey: String = event.pubkey.as_str().to_owned();
        let kind: u64 = event.kind;
        let id: String = event.id.as_str().to_owned();
        let param: String = parameter.clone();
        let count: usize = spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(&sql)?;
            stmt.raw_bind_parameter(1, &pubkey)?;
            stmt.raw_bind_parameter(2, kind)?;
            stmt.raw_bind_parameter(3, &id)?;
            stmt.raw_bind_parameter(4, &param)?;
            let mut rows = stmt.raw_query();
            if let Some(row) = rows.next()? {
                return Ok(row.get(0)?);
            }
            Ok::<usize, Error>(0)
        })
        .await??;

        // If nothing is newer, save this event
        if count == 0 {
            Self::insert(event).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /*
        pub async fn get_author(id: IdHex) -> Result<Option<PublicKeyHex>, Error> {
            let sql = "SELECT pubkey FROM event WHERE id=?";

            spawn_blocking(move || {
                let db = GLOBALS.db.blocking_lock();
                let mut stmt = db.prepare(sql)?;
                let mut rows = stmt.query_map([id.0], |row| row.get(0))?;
                if let Some(row) = rows.next() {
                    return Ok(Some(PublicKeyHex(row?)));
                }
                Ok(None)
            })
            .await?
    }
        */
}

/*
fn repeat_vars(count: usize) -> String {
    assert_ne!(count, 0);
    let mut s = "?,".repeat(count);
    // Remove trailing comma
    s.pop();
    s
}
*/
