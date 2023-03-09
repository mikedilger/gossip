use crate::error::Error;
use crate::globals::GLOBALS;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DbEventTag {
    pub event: String,
    pub seq: u64,
    pub label: Option<String>,
    pub field0: Option<String>,
    pub field1: Option<String>,
    pub field2: Option<String>,
    pub field3: Option<String>,
}

impl DbEventTag {
    pub async fn insert(event_tag: DbEventTag) -> Result<(), Error> {
        let sql =
            "INSERT OR IGNORE INTO event_tag (event, seq, label, field0, field1, field2, field3) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";

        let db = GLOBALS.db.get()?;

        let mut stmt = db.prepare(sql)?;
        stmt.execute((
            &event_tag.event,
            &event_tag.seq,
            &event_tag.label,
            &event_tag.field0,
            &event_tag.field1,
            &event_tag.field2,
            &event_tag.field3,
        ))?;

        Ok(())
    }
}
