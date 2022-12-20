use crate::error::Error;
use crate::globals::GLOBALS;
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

#[derive(Debug, Serialize, Deserialize)]
pub struct DbContact {
    pub source: String,
    pub contact: String,
    pub relay: Option<String>,
    pub petname: Option<String>,
}

impl DbContact {
    #[allow(dead_code)]
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbContact>, Error> {
        let sql = "SELECT source, contact, relay, petname FROM contact".to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
        };

        let output: Result<Vec<DbContact>, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                Ok(DbContact {
                    source: row.get(0)?,
                    contact: row.get(1)?,
                    relay: row.get(2)?,
                    petname: row.get(3)?,
                })
            })?;

            let mut output: Vec<DbContact> = Vec::new();
            for row in rows {
                output.push(row?);
            }
            Ok(output)
        })
        .await?;

        output
    }

    #[allow(dead_code)]
    pub async fn insert(contact: DbContact) -> Result<(), Error> {
        let sql = "INSERT OR IGNORE INTO contact (source, contact, relay, petname) \
             VALUES (?1, ?2, ?3, ?4)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(sql)?;
            stmt.execute((
                &contact.source,
                &contact.contact,
                &contact.relay,
                &contact.petname,
            ))?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn delete(criteria: &str) -> Result<(), Error> {
        let sql = format!("DELETE FROM contact WHERE {}", criteria);

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            db.execute(&sql, [])?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }
}
