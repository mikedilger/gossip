use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Event, IdHex, PublicKeyHex};
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

/*
overlord:
    search

process:
    replace
    replace_parameterized
    insert
*/

// THIS IS GOING AWAY we will use nostr_types::Event instead
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DbEvent {
    pub id: IdHex,            // will be Id
    pub raw: String,          // -- gone
    pub pubkey: PublicKeyHex, // will be PublicKey
    pub created_at: i64,      // will be Unixtime
    pub kind: u64,            // will be EventType
    pub content: String,
    pub ots: Option<String>,
    // will have sig
    // will have tags: Vec<Tag>
}

impl DbEvent {
    pub async fn search(text: &str) -> Result<Vec<Event>, Error> {
        let sql = format!("SELECT raw FROM event WHERE (kind=1 OR kind=30023) AND (\
                           content LIKE '%{text}%' \
                           OR \
                           id IN (SELECT event FROM event_tag WHERE label IN ('t', 'subject', 'summary', 'title') AND field0 like '%{text}%') \
                           ) \
                           ORDER BY created_at DESC");

        let output: Result<Vec<Event>, Error> = spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(&sql)?;
            let mut rows = stmt.query([])?;
            let mut output: Vec<Event> = Vec::new();
            while let Some(row) = rows.next()? {
                let raw: String = row.get(0)?;
                let event: Event = match serde_json::from_str(&raw) {
                    Ok(e) => e,
                    Err(_) => continue, // ignore the error, keep searching
                };
                output.push(event);
            }
            Ok(output)
        })
        .await?;

        output
    }

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
}
