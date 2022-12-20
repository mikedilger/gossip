use super::{DbEventSeen, DbEventTag};
use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_proto::{Event, IdHex, PublicKeyHex, Unixtime, Url};
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
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
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

    pub async fn insert(event: DbEvent) -> Result<(), Error> {
        let sql = "INSERT OR IGNORE INTO event (id, raw, pubkey, created_at, kind, content, ots) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                &event.id.0,
                &event.raw,
                &event.pubkey.0,
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

    #[allow(dead_code)]
    pub async fn delete(criteria: &str) -> Result<(), Error> {
        let sql = format!("DELETE FROM event WHERE {}", criteria);

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            db.execute(&sql, [])?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn get_author(id: IdHex) -> Result<Option<PublicKeyHex>, Error> {
        let sql = "SELECT pubkey FROM event WHERE id=?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(sql)?;
            let mut rows = stmt.query_map([id.0], |row| row.get(0))?;
            if let Some(row) = rows.next() {
                return Ok(Some(PublicKeyHex(row?)));
            }
            Ok(None)
        })
        .await?
    }

    pub async fn save_nostr_event(event: &Event, seen_on: Option<Url>) -> Result<(), Error> {
        // Convert a nostr Event into a DbEvent
        let db_event = DbEvent {
            id: event.id.into(),
            raw: serde_json::to_string(&event)?,
            pubkey: event.pubkey.into(),
            created_at: event.created_at.0,
            kind: event.kind.into(),
            content: event.content.clone(),
            ots: event.ots.clone(),
        };

        // Save into event table
        DbEvent::insert(db_event).await?;

        // Save the tags into event_tag table
        for (seq, tag) in event.tags.iter().enumerate() {
            // convert to vec of strings
            let v: Vec<String> = serde_json::from_str(&serde_json::to_string(&tag)?)?;

            let db_event_tag = DbEventTag {
                event: event.id.as_hex_string(),
                seq: seq as u64,
                label: v.get(0).cloned(),
                field0: v.get(1).cloned(),
                field1: v.get(2).cloned(),
                field2: v.get(3).cloned(),
                field3: v.get(4).cloned(),
            };
            DbEventTag::insert(db_event_tag).await?;
        }

        // Save the event into event_seen table
        if let Some(url) = seen_on {
            let db_event_seen = DbEventSeen {
                event: event.id.as_hex_string(),
                relay: url.0,
                when_seen: Unixtime::now()?.0 as u64,
            };
            DbEventSeen::replace(db_event_seen).await?;
        }

        Ok(())
    }
}
