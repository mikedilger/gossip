use crate::error::Error;
use crate::globals::GLOBALS;
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

#[derive(Debug, Serialize, Deserialize)]
pub struct DbEventRelationship {
    pub original: String,
    pub referring: String,
    pub relationship: String,
    pub content: Option<String>,
}

impl DbEventRelationship {
    pub async fn insert(&self) -> Result<(), Error> {
        let original = self.original.clone();
        let referring = self.referring.clone();
        let relationship = self.relationship.clone();
        let content = self.content.clone();
        let sql = "INSERT OR IGNORE INTO event_relationship (original, referring, relationship, content) VALUES (?, ?, ?, ?)";
        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare(sql)?;
            stmt.execute((&original, &referring, &relationship, &content))?;
            Ok::<(), Error>(())
        })
        .await??;
        Ok(())
    }

    /*
        pub async fn get_events_referring_to(id: Id) -> Result<Vec<DbEventRelationship>, Error> {
            let sql =
                "SELECT referring, relationship, content FROM event_relationship WHERE original=?";
            let output: Result<Vec<DbEventRelationship>, Error> = spawn_blocking(move || {
                let maybe_db = GLOBALS.db.blocking_lock();
                let db = maybe_db.as_ref().unwrap();
                let mut stmt = db.prepare(sql)?;
                let rows = stmt.query_map([id.as_hex_string()], |row| {
                    Ok(DbEventRelationship {
                        original: id.as_hex_string(),
                        referring: row.get(0)?,
                        relationship: row.get(1)?,
                        content: row.get(2)?,
                    })
                })?;
                let mut output: Vec<DbEventRelationship> = Vec::new();
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
