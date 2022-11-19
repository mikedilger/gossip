use crate::{Error, GLOBALS};
use serde::{Deserialize, Serialize};
use tauri::async_runtime::spawn_blocking;

#[derive(Debug, Serialize, Deserialize)]
pub struct DbEvent {
    pub id: String,
    pub public_key: String,
    pub created_at: i64,
    pub kind: u8,
    pub content: String,
    pub ots: Option<String>
}

impl DbEvent {
    #[allow(dead_code)]
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbEvent>, Error> {
        let sql =
            "SELECT id, public_key, created_at, kind, content, ots FROM event".to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
        };

        let output: Result<Vec<DbEvent>, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                Ok(DbEvent {
                    id: row.get(0)?,
                    public_key: row.get(1)?,
                    created_at: row.get(2)?,
                    kind: row.get(3)?,
                    content: row.get(4)?,
                    ots: row.get(5)?,
                })
            })?;

            let mut output: Vec<DbEvent> = Vec::new();
            for row in rows {
                output.push(row?);
            }
            Ok(output)
        })
        .await?;

        output
    }

    #[allow(dead_code)]
    pub async fn insert(event: DbEvent) -> Result<(), Error> {
        let sql =
            "INSERT OR IGNORE INTO event (id, public_key, created_at, kind, content, ots) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            stmt.execute((
                &event.id,
                &event.public_key,
                &event.created_at,
                &event.kind,
                &event.content,
                &event.ots
            ))?;
            Ok::<(), Error>(())
        }).await??;

        Ok(())
    }
}
