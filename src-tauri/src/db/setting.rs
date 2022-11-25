use crate::{Error, GLOBALS};
use serde::{Deserialize, Serialize};
use rusqlite::ToSql;
use tauri::async_runtime::spawn_blocking;

#[derive(Debug, Serialize, Deserialize)]
pub struct DbSetting {
    pub key: String,
    pub value: String
}

impl DbSetting {
    #[allow(dead_code)]
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbSetting>, Error> {
        let sql =
            "SELECT key, value FROM settings".to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
        };

        let output: Result<Vec<DbSetting>, Error> = spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                Ok(DbSetting {
                    key: row.get(0)?,
                    value: row.get(1)?,
                })
            })?;

            let mut output: Vec<DbSetting> = Vec::new();
            for row in rows {
                output.push(row?);
            }
            Ok(output)
        })
        .await?;

        output
    }

    #[allow(dead_code)]
    pub async fn fetch_setting(key: &str) -> Result<Option<String>, Error> {
        let db_settings = DbSetting::fetch(
            Some(&format!("key='{}'",key))
        ).await?;

        if db_settings.len() == 0 {
            Ok(None)
        } else {
            Ok(Some(db_settings[0].value.clone()))
        }
    }

    #[allow(dead_code)]
    pub async fn fetch_setting_or_default(key: &str, default: &str)
                                          -> Result<String, Error>
    {
        let db_settings = DbSetting::fetch(
            Some(&format!("key='{}'",key))
        ).await?;

        if db_settings.len() == 0 {
            Ok(default.to_string())
        } else {
            Ok(db_settings[0].value.clone())
        }
    }

    #[allow(dead_code)]
    pub async fn fetch_setting_u64_or_default(key: &str, default: u64)
                                              -> Result<u64, Error>
    {
        let db_settings = DbSetting::fetch(
            Some(&format!("key='{}'",key))
        ).await?;

        if db_settings.len() == 0 {
            Ok(default)
        } else {
            Ok(db_settings[0].value.parse::<u64>().unwrap_or(default))
        }
    }

    #[allow(dead_code)]
    pub async fn insert(setting: DbSetting) -> Result<(), Error> {
        let sql =
            "INSERT OR IGNORE INTO settings (key, value) \
             VALUES (?1, ?2)";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            stmt.execute((
                &setting.key,
                &setting.value
            ))?;
            Ok::<(), Error>(())
        }).await??;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn update<T: ToSql + Send + 'static>(key: String, value: T) -> Result<(), Error> {
        let sql =
            "UPDATE settings SET value=? WHERE key=?";

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();

            let mut stmt = db.prepare(&sql)?;
            stmt.execute((&value, &key))?;
            Ok::<(), Error>(())
        }).await??;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn delete(criteria: &str) -> Result<(), Error> {
        let sql = format!("DELETE FROM settings WHERE {}", criteria);

        spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            db.execute(&sql, [])?;
            Ok::<(), Error>(())
        }).await??;

        Ok(())
    }
}
