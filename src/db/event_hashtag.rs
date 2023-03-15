use crate::error::Error;
use crate::globals::GLOBALS;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DbEventHashtag {
    pub event: String,
    pub hashtag: String,
}

impl DbEventHashtag {
    pub async fn insert(&self) -> Result<(), Error> {
        let event = self.event.clone();
        let hashtag = self.hashtag.clone();
        let sql = "INSERT OR IGNORE INTO event_hashtag (event, hashtag) VALUES (?, ?)";

        let pool = GLOBALS.db.clone();
        let db = pool.get()?;
        let mut stmt = db.prepare(sql)?;
        stmt.execute((&event, &hashtag))?;
        Ok(())
    }
}
