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

    /*
        pub async fn get_events_with_hashtag(hashtag: String) -> Result<Vec<DbEventHashtag>, Error> {
            let sql = "SELECT event FROM event_hashtag WHERE hashtag=?";
            let output: Result<Vec<DbEventHashtag>, Error> = spawn_blocking(move || {
                let maybe_db = GLOBALS.db.blocking_lock();
                let db = maybe_db.as_ref().unwrap();
                let mut stmt = db.prepare(sql)?;
                let rows = stmt.query_map([hashtag.clone()], |row| {
                    Ok(DbEventHashtag {
                        event: row.get(0)?,
                        hashtag: hashtag.clone(),
                    })
                })?;
                let mut output: Vec<DbEventHashtag> = Vec::new();
                for row in rows {
                    output.push(row?);
                }
                Ok(output)
            })
            .await?;
            output
    }
        */
}
