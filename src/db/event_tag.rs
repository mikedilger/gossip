use crate::error::Error;
use crate::globals::GLOBALS;
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

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
    /*
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbEventTag>, Error> {
        let sql =
            "SELECT event, seq, label, field0, field1, field2, field3 FROM event_tag".to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
        };

        let output: Result<Vec<DbEventTag>, Error> = spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                Ok(DbEventTag {
                    event: row.get(0)?,
                    seq: row.get(1)?,
                    label: row.get(2)?,
                    field0: row.get(3)?,
                    field1: row.get(4)?,
                    field2: row.get(5)?,
                    field3: row.get(6)?,
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
     */

    pub async fn insert(event_tag: DbEventTag) -> Result<(), Error> {
        let sql =
            "INSERT OR IGNORE INTO event_tag (event, seq, label, field0, field1, field2, field3) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";

        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
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
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    /*
        pub async fn delete(criteria: &str) -> Result<(), Error> {
            let sql = format!("DELETE FROM event_tag WHERE {}", criteria);

            spawn_blocking(move || {
                let db = GLOBALS.db.blocking_lock();
                db.execute(&sql, [])?;
                Ok::<(), Error>(())
            })
            .await??;

            Ok(())
    }
        */
}
