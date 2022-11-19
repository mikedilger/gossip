use crate::{Error, GLOBALS};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DbPerson {
    pub public_key: String,
    pub name: Option<String>,
    pub about: Option<String>,
    pub picture: Option<String>,
    pub nip05: Option<String>,
    pub following: u8
}

impl DbPerson {
    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbPerson>, Error> {
        let maybe_db = GLOBALS.db.lock().await;
        let db = maybe_db.as_ref().unwrap();

        let sql = "SELECT public_key, name, about, picture, nip05, following FROM person".to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit)
        };

        let mut stmt = db.prepare(&sql)?;

        let rows = stmt.query_map([], |row| {
            Ok(DbPerson {
                public_key: row.get(0)?,
                name: row.get(1)?,
                about: row.get(2)?,
                picture: row.get(3)?,
                nip05: row.get(4)?,
                following: row.get(5)?,
            })
        })?;

        let mut output: Vec<DbPerson> = Vec::new();
        for row in rows {
            output.push(row?);
        }

        Ok(output)
    }
}
