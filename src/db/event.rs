use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Event, EventKind, IdHex, PublicKeyHex};
use serde::{Deserialize, Serialize};

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

        let db = GLOBALS.db.get()?;

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
    }

    pub async fn fetch_latest_contact_list(
        pubkeyhex: PublicKeyHex,
    ) -> Result<Option<Event>, Error> {
        let sql = "SELECT raw FROM event WHERE event.kind=3 AND event.pubkey=? ORDER BY created_at DESC LIMIT 1";

        let db = GLOBALS.db.get()?;

        let mut stmt = db.prepare(sql)?;
        stmt.raw_bind_parameter(1, pubkeyhex.as_str())?;
        let mut rows = stmt.raw_query();
        let mut events: Vec<Event> = Vec::new();
        while let Some(row) = rows.next()? {
            let raw: String = row.get(0)?;
            let event: Event = serde_json::from_str(&raw)?;
            events.push(event);
        }

        let mut temp = events.drain(..);
        Ok(temp.next())
    }

    pub async fn fetch_relay_lists() -> Result<Vec<Event>, Error> {
        // FIXME, only get the last per pubkey
        let sql = "SELECT raw FROM event WHERE event.kind=10002";

        let db = GLOBALS.db.get()?;

        let mut stmt = db.prepare(sql)?;
        let mut rows = stmt.raw_query();
        let mut events: Vec<Event> = Vec::new();
        while let Some(row) = rows.next()? {
            let raw: String = row.get(0)?;
            let event: Event = serde_json::from_str(&raw)?;
            events.push(event);
        }

        Ok(events)
    }

    pub async fn fetch_reply_related(since: i64) -> Result<Vec<DbEvent>, Error> {
        let public_key: PublicKeyHex = match GLOBALS.signer.public_key() {
            None => return Ok(vec![]),
            Some(pk) => pk.into(),
        };

        let mut kinds = vec![EventKind::TextNote, EventKind::EventDeletion];
        if GLOBALS.settings.read().direct_messages {
            kinds.push(EventKind::EncryptedDirectMessage);
        }
        if GLOBALS.settings.read().reposts {
            kinds.push(EventKind::Repost);
        }
        if GLOBALS.settings.read().reactions {
            kinds.push(EventKind::Reaction);
        }

        let kinds: Vec<String> = kinds
            .iter()
            .map(|e| <EventKind as Into<u64>>::into(*e))
            .map(|e| e.to_string())
            .collect();
        let kinds = kinds.join(",");

        let sql = format!(
            "SELECT id, raw, pubkey, created_at, kind, content, ots FROM event \
             LEFT JOIN event_tag ON event.id=event_tag.event \
             WHERE event.kind IN ({}) \
             AND event_tag.label='p' AND event_tag.field0=? \
             AND created_at > ? \
             ORDER BY created_at ASC",
            kinds
        );

        let db = GLOBALS.db.get()?;

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
    }

    pub async fn insert(event: DbEvent) -> Result<(), Error> {
        let sql = "INSERT OR IGNORE INTO event (id, raw, pubkey, created_at, kind, content, ots) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";

        let db = GLOBALS.db.get()?;
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

        Ok(())
    }

}
