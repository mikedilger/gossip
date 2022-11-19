use crate::{Error, GLOBALS};
use rusqlite::Connection;

mod relay;
pub use relay::DbRelay;

mod person;
pub use person::DbPerson;

mod person_relay;
pub use person_relay::DbPersonRelay;

/*
#[derive(Debug)]
struct DbbSetting {
    key: String,
    value: String
}

#[derive(Debug)]
struct DbPersonContact {
    person: String,
    contact: String,
    relay: Option<String>,
    petname: Option<String>,
}

#[derive(Debug)]
struct DbEvent {
    id: String,
    public_key: String,
    created_at: i64,
    kind: u8,
    content: String,
    ots: Option<String>
}

#[derive(Debug)]
struct DbEventTag {
    event: String,
    label: String,
    field0: Option<String>,
    field1: Option<String>,
    field2: Option<String>,
    field3: Option<String>,
}
 */

pub async fn check_and_upgrade() -> Result<(), Error> {
    let maybe_db = GLOBALS.db.lock().await;
    let db = maybe_db.as_ref().unwrap();

    // Load the current version
    match db.query_row(
        "SELECT value FROM settings WHERE key=?",
        ["version"],
        |row| row.get::<usize, String>(0),
    ) {
        Ok(v) => upgrade(db, v.parse::<u16>().unwrap()),
        Err(_e) => {
            // Check the error first!
            upgrade(db, 0)
        }
    }
}

macro_rules! apply_sql {
    ($db:ident, $version:ident, $thisversion:expr, $file:expr) => {{
        if $version < $thisversion {
            log::info!("Upgrading database to version {}", $thisversion);
            $db.execute_batch(include_str!($file))?;
            $db.execute(
                &format!(
                    "UPDATE settings SET value='{}' WHERE key='version'",
                    $thisversion
                ),
                (),
            )?;
            $version = $thisversion;
        }
    }};
}

fn upgrade(db: &Connection, mut version: u16) -> Result<(), Error> {
    apply_sql!(db, version, 1, "schema1.sql");

    log::info!("Database is at version {}", version);

    Ok(())
}
