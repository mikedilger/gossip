use crate::error::Error;
use crate::globals::GLOBALS;
use crate::profile::Profile;
use rusqlite::Connection;

pub fn init_database() -> Result<Connection, Error> {
    let profile_dir = Profile::current()?.profile_dir;

    // Connect to (or create) our database
    let mut db_path = profile_dir;
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

    Ok(connection)
}

// This sets up the database
#[allow(clippy::or_fun_call)]
pub fn setup_database() -> Result<(), Error> {
    let db = GLOBALS.db.blocking_lock();

    // Enforce foreign key relationships
    db.pragma_update(None, "foreign_keys", "ON")?;

    // Performance:
    db.pragma_update(None, "journal_mode", "WAL")?;
    db.pragma_update(None, "synchronous", "normal")?;
    db.pragma_update(None, "temp_store", "memory")?;
    db.pragma_update(None, "mmap_size", "268435456")?; // 1024 * 1024 * 256

    Ok(())
}
