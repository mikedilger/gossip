use crate::error::Error;
use crate::globals::GLOBALS;
use serde::{Deserialize, Serialize};

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

        let db = GLOBALS.db.get()?;
        let mut stmt = db.prepare(sql)?;
        stmt.execute((&original, &refers_to, &relationship, &content))?;
        Ok(())
    }
}
