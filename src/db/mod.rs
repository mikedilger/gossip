mod event;
pub use event::DbEvent;

mod event_seen;
pub use event_seen::DbEventSeen;

mod event_hashtag;
pub use event_hashtag::DbEventHashtag;

mod event_tag;
pub use event_tag::DbEventTag;

mod event_relationship;
pub use event_relationship::DbEventRelationship;

mod relay;
pub use relay::DbRelay;

mod contact;
pub use contact::DbContact;

mod person_relay;
pub use person_relay::DbPersonRelay;

mod setting;
pub use setting::DbSetting;

use crate::error::Error;
use crate::globals::GLOBALS;
use rusqlite::Connection;
use std::fs;
use tokio::task;

// This sets up the database
#[allow(clippy::or_fun_call)]
pub fn setup_database() -> Result<(), Error> {
    let mut data_dir = dirs::data_dir()
        .ok_or::<Error>("Cannot find a directory to store application data.".into())?;
    data_dir.push("gossip");

    // Create our data directory only if it doesn't exist
    fs::create_dir_all(&data_dir)?;

    // Connect to (or create) our database
    let mut db_path = data_dir.clone();
    db_path.push("gossip.sqlite");
    let connection = Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
            | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX
            | rusqlite::OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;

    // Turn on foreign keys
    connection.execute("PRAGMA foreign_keys = ON", ())?;

    // Save the connection globally
    {
        let mut db = GLOBALS.db.blocking_lock();
        *db = Some(connection);
    }

    // Check and upgrade our data schema
    check_and_upgrade()?;

    Ok(())
}

fn check_and_upgrade() -> Result<(), Error> {
    let maybe_db = GLOBALS.db.blocking_lock();
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
            tracing::info!("Upgrading database to version {}", $thisversion);
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
    let current_version = 21;
    if version > current_version {
        panic!(
            "Database version {} is newer than this binary which expects version {}.",
            version, current_version
        );
    }

    // note to developers: we cannot make this into a loop because include_str! included
    // by apply_sql! requires a static string, not a dynamically formatted one.
    apply_sql!(db, version, 1, "schema1.sql");
    apply_sql!(db, version, 2, "schema2.sql");
    apply_sql!(db, version, 3, "schema3.sql");
    apply_sql!(db, version, 4, "schema4.sql");
    apply_sql!(db, version, 5, "schema5.sql");
    apply_sql!(db, version, 6, "schema6.sql");
    apply_sql!(db, version, 7, "schema7.sql");
    apply_sql!(db, version, 8, "schema8.sql");
    apply_sql!(db, version, 9, "schema9.sql");
    apply_sql!(db, version, 10, "schema10.sql");
    apply_sql!(db, version, 11, "schema11.sql");
    apply_sql!(db, version, 12, "schema12.sql");
    apply_sql!(db, version, 13, "schema13.sql");
    apply_sql!(db, version, 14, "schema14.sql");
    apply_sql!(db, version, 15, "schema15.sql");
    apply_sql!(db, version, 16, "schema16.sql");
    apply_sql!(db, version, 17, "schema17.sql");
    apply_sql!(db, version, 18, "schema18.sql");
    apply_sql!(db, version, 19, "schema19.sql");
    apply_sql!(db, version, 20, "schema20.sql");
    apply_sql!(db, version, 21, "schema21.sql");
    tracing::info!("Database is at version {}", version);
    Ok(())
}

pub async fn prune() -> Result<(), Error> {
    task::spawn_blocking(move || {
        let maybe_db = GLOBALS.db.blocking_lock();
        let db = maybe_db.as_ref().unwrap();
        db.execute_batch(include_str!("prune.sql"))?;
        Ok::<(), Error>(())
    })
    .await??;

    *GLOBALS.status_message.write().await = "Database prune has completed.".to_owned();

    Ok(())
}
