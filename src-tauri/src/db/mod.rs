use crate::{Error, GLOBALS};
use rusqlite::Connection;

mod event;
pub use event::DbEvent;

mod event_seen;
pub use event_seen::DbEventSeen;

mod event_tag;
pub use event_tag::DbEventTag;

mod relay;
pub use relay::DbRelay;

mod person;
pub use person::DbPerson;

mod contact;
pub use contact::DbContact;

mod person_relay;
pub use person_relay::DbPersonRelay;

mod setting;
pub use setting::DbSetting;

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
