use crate::{Error, GLOBALS};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DbRelay {
    pub url: String,
    pub last_up: Option<String>,
    pub last_try: Option<String>,
    pub last_fetched: Option<String>
}

impl DbRelay {
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbRelay>, Error> {
        let maybe_db = GLOBALS.db.lock().await;
        let db = maybe_db.as_ref().unwrap();

        let sql = "SELECT url, last_up, last_try, last_fetched FROM relay".to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
        };

        let mut stmt = db.prepare(&sql)?;

        let rows = stmt.query_map([], |row| {
            Ok(DbRelay {
                url: row.get(0)?,
                last_up: row.get(1)?,
                last_try: row.get(2)?,
                last_fetched: row.get(3)?,
            })
        })?;

        let mut output: Vec<DbRelay> = Vec::new();
        for row in rows {
            output.push(row?);
        }

        Ok(output)
    }
}
