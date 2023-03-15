use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::Id;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DbEventFlags {
    pub event: Id,
    pub viewed: bool,
}

impl DbEventFlags {
    pub async fn load_all_viewed() -> Result<Vec<Id>, Error> {
        let sql = "SELECT event FROM event_flags WHERE viewed=1".to_owned();
        let pool = GLOBALS.db.clone();
        let db = pool.get()?;
        let mut stmt = db.prepare(&sql)?;
        let mut output: Vec<Id> = Vec::new();
        let mut rows = stmt.raw_query();
        while let Some(row) = rows.next()? {
            let s: String = row.get(0)?;
            if let Ok(id) = Id::try_from_hex_string(&s) {
                output.push(id);
            }
        }
        Ok(output)
    }

    pub async fn mark_all_as_viewed(ids: Vec<Id>) -> Result<(), Error> {
        let sql = "INSERT INTO event_flags (event, viewed) VALUES (?, 1) \
                   ON CONFLICT(event) DO UPDATE SET viewed=1";
        let pool = GLOBALS.db.clone();
        let db = pool.get()?;
        let mut stmt = db.prepare(sql)?;
        for id in ids {
            stmt.raw_bind_parameter(1, id.as_hex_string())?;
            let _ = stmt.raw_execute(); // IGNORE errors, this is not critical.
        }

        Ok(())
    }
}
