use crate::error::Error;
use crate::globals::GLOBALS;
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

#[derive(Debug, Serialize, Deserialize)]
pub struct DbEventRelationship {
    pub original: String,
    pub refers_to: String,
    pub relationship: String,
    pub content: Option<String>,
}

impl DbEventRelationship {
    pub async fn insert(&self) -> Result<(), Error> {
        let original = self.original.clone();
        let refers_to = self.refers_to.clone();
        let relationship = self.relationship.clone();
        let content = self.content.clone();
        let sql = "INSERT OR IGNORE INTO event_relationship (original, refers_to, relationship, content) VALUES (?, ?, ?, ?)";

        let pool = GLOBALS.db.clone();
        spawn_blocking(move || {
            let db = pool.get()?;
            let mut stmt = db.prepare(sql)?;
            stmt.execute((&original, &refers_to, &relationship, &content))?;
            Ok::<(), Error>(())
        })
        .await??;
        Ok(())
    }

    /*
        pub async fn get_events_refers_to(id: Id) -> Result<Vec<DbEventRelationship>, Error> {
            let sql =
                "SELECT refers_to, relationship, content FROM event_relationship WHERE original=?";
            let output: Result<Vec<DbEventRelationship>, Error> = spawn_blocking(move || {
                let maybe_db = GLOBALS.db.blocking_lock();
                let db = maybe_db.as_ref().unwrap();
                let mut stmt = db.prepare(sql)?;
                let rows = stmt.query_map([id.as_hex_string()], |row| {
                    Ok(DbEventRelationship {
                        original: id.as_hex_string(),
                        refers_to: row.get(0)?,
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
