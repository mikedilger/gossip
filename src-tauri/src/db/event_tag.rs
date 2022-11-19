use crate::{Error, GLOBALS};
use serde::{Deserialize, Serialize};
use tauri::async_runtime::spawn_blocking;

#[derive(Debug, Serialize, Deserialize)]
pub struct DbEventTag {
    pub event: String,
    pub label: String,
    pub field0: Option<String>,
    pub field1: Option<String>,
    pub field2: Option<String>,
    pub field3: Option<String>,
}

impl DbEventTag {
    #[allow(dead_code)]
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbEventTag>, Error> {
        let sql =
            "SELECT event, label, field0, field1, field2, field3 FROM event_tag".to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
        };

        let output: Result<Vec<DbEventTag>, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                Ok(DbEventTag {
                    event: row.get(0)?,
                    label: row.get(1)?,
                    field0: row.get(2)?,
                    field1: row.get(3)?,
                    field2: row.get(4)?,
                    field3: row.get(5)?,
                })
            })?;

            let mut output: Vec<DbEventTag> = Vec::new();
            for row in rows {
                output.push(row?);
            }
            Ok(output)
        })
        .await?;

        output
    }

    #[allow(dead_code)]
    pub async fn insert(event_tag: DbEventTag) -> Result<(), Error> {
        let sql =
            "INSERT OR IGNORE INTO event_tag (event, label, field0, field1, field2, field3) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            stmt.execute((
                &event_tag.event,
                &event_tag.label,
                &event_tag.field0,
                &event_tag.field1,
                &event_tag.field2,
                &event_tag.field3
            ))?;
            Ok::<(), Error>(())
        }).await??;

        Ok(())
    }
}
