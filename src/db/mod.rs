use crate::error::Error;
use crate::globals::GLOBALS;
use crate::profile::Profile;
use fallible_iterator::FallibleIterator;
use rusqlite::Connection;
use std::sync::atomic::Ordering;
use tokio::task;

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
    // Check and upgrade our data schema
    check_and_upgrade()?;

    // Normalize URLs
    normalize_urls()?;

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

fn check_and_upgrade() -> Result<(), Error> {
    let db = GLOBALS.db.blocking_lock();
    match db.query_row(
        "SELECT schema_version FROM local_settings LIMIT 1",
        [],
        |row| row.get::<usize, usize>(0),
    ) {
        Ok(version) => upgrade(&db, version),
        Err(e) => {
            if let rusqlite::Error::SqliteFailure(_, Some(ref s)) = e {
                if s.contains("no such table") {
                    return old_check_and_upgrade(&db);
                }
            }
            Err(e.into())
        }
    }
}

fn old_check_and_upgrade(db: &Connection) -> Result<(), Error> {
    match db.query_row(
        "SELECT value FROM settings WHERE key='version'",
        [],
        |row| row.get::<usize, String>(0),
    ) {
        Ok(v) => {
            let version = v.parse::<usize>().unwrap();
            if version < 2 {
                GLOBALS.first_run.store(true, Ordering::Relaxed);
            }
            upgrade(db, version)
        }
        Err(_e) => {
            GLOBALS.first_run.store(true, Ordering::Relaxed);
            // Check the error first!
            upgrade(db, 0)
        }
    }
}

fn upgrade(db: &Connection, mut version: usize) -> Result<(), Error> {
    if version > UPGRADE_SQL.len() {
        panic!(
            "Database version {} is newer than this binary which expects version {}.",
            version,
            UPGRADE_SQL.len()
        );
    }

    // Disable foreign key checks during upgrades (some foreign keys relationships
    // may be broken, we don't want that to stop us)
    db.pragma_update(None, "foreign_keys", "OFF")?;

    while version < UPGRADE_SQL.len() {
        tracing::info!("Upgrading database to version {}", version + 1);
        db.execute_batch(UPGRADE_SQL[version + 1 - 1])?;
        version += 1;
        if version < 24 {
            // 24 is when we switched to local_settings
            db.execute(
                "UPDATE settings SET value=? WHERE key='version'",
                (version,),
            )?;
        } else {
            db.execute("UPDATE local_settings SET schema_version=?", (version,))?;
        }
    }

    db.pragma_update(None, "foreign_keys", "ON")?;

    tracing::info!("Database is at version {}", version);

    Ok(())
}

pub async fn prune() -> Result<(), Error> {
    task::spawn_blocking(move || {
        let db = GLOBALS.db.blocking_lock();
        db.execute_batch(include_str!("sql/prune.sql"))?;
        Ok::<(), Error>(())
    })
    .await??;

    GLOBALS
        .status_queue
        .write()
        .write("Database prune has completed.".to_owned());

    Ok(())
}

fn normalize_urls() -> Result<(), Error> {
    let db = GLOBALS.db.blocking_lock();
    let urls_are_normalized: bool = db.query_row(
        "SELECT urls_are_normalized FROM local_settings LIMIT 1",
        [],
        |row| row.get::<usize, bool>(0),
    )?;

    if urls_are_normalized {
        return Ok(());
    }

    tracing::info!("Normalizing Database URLs (this will take some time)");

    db.pragma_update(None, "foreign_keys", "OFF")?;

    // relay.url
    let sql = "SELECT url FROM relay";
    let mut stmt = db.prepare(sql)?;
    let rows = stmt.query([])?;
    let all_rows: Vec<String> = rows.map(|row| row.get(0)).collect()?;
    for urlkey in all_rows.iter() {
        match nostr_types::RelayUrl::try_from_str(urlkey) {
            Ok(url) => {
                let urlstr = url.as_str().to_owned();
                // Update if not equal
                if *urlkey != urlstr {
                    // this one is too verbose
                    // tracing::debug!("Updating non-canonical URL from {} to {}", urlkey, urlstr);
                    let usql = "UPDATE relay SET url=? WHERE url=?";
                    let mut stmt = db.prepare(usql)?;
                    if let Err(e) = stmt.execute((&urlstr, urlkey)) {
                        if let rusqlite::Error::SqliteFailure(_, Some(ref s)) = e {
                            if s.contains("constraint failed") {
                                // Delete this row instead, there is some other row that is already
                                // what we are trying to turn this row into
                                let dsql = "DELETE FROM relay WHERE url=?";
                                let mut stmt = db.prepare(dsql)?;
                                stmt.execute((&urlkey,))?;
                            }
                        } else {
                            return Err(e.into());
                        }
                    }

                    let usql = "UPDATE person_relay SET relay=? WHERE relay=?";
                    let mut stmt = db.prepare(usql)?;
                    stmt.execute((&urlstr, urlkey))?;

                    let usql = "UPDATE event_relay SET relay=? WHERE relay=?";
                    let mut stmt = db.prepare(usql)?;
                    stmt.execute((&urlstr, urlkey))?;
                }
            }
            Err(_) => {
                // Delete if did not parse properly
                tracing::debug!("Deleting invalid relay url {}", urlkey);

                let dsql = "DELETE FROM relay WHERE url=?";
                let mut stmt = db.prepare(dsql)?;
                stmt.execute((urlkey,))?;

                let dsql = "DELETE FROM person_relay WHERE relay=?";
                let mut stmt = db.prepare(dsql)?;
                stmt.execute((urlkey,))?;

                let dsql = "DELETE FROM event_relay WHERE relay=?";
                let mut stmt = db.prepare(dsql)?;
                stmt.execute((urlkey,))?;
            }
        };
    }

    let sql = "UPDATE local_settings SET urls_are_normalized=1";
    let mut stmt = db.prepare(sql)?;
    stmt.execute(())?;

    Ok(())
}

const UPGRADE_SQL: [&str; 37] = [
    include_str!("sql/schema1.sql"),
    include_str!("sql/schema2.sql"),
    include_str!("sql/schema3.sql"),
    include_str!("sql/schema4.sql"),
    include_str!("sql/schema5.sql"),
    include_str!("sql/schema6.sql"),
    include_str!("sql/schema7.sql"),
    include_str!("sql/schema8.sql"),
    include_str!("sql/schema9.sql"),
    include_str!("sql/schema10.sql"),
    include_str!("sql/schema11.sql"),
    include_str!("sql/schema12.sql"),
    include_str!("sql/schema13.sql"),
    include_str!("sql/schema14.sql"),
    include_str!("sql/schema15.sql"),
    include_str!("sql/schema16.sql"),
    include_str!("sql/schema17.sql"),
    include_str!("sql/schema18.sql"),
    include_str!("sql/schema19.sql"),
    include_str!("sql/schema20.sql"),
    include_str!("sql/schema21.sql"),
    include_str!("sql/schema22.sql"),
    include_str!("sql/schema23.sql"),
    include_str!("sql/schema24.sql"),
    include_str!("sql/schema25.sql"),
    include_str!("sql/schema26.sql"),
    include_str!("sql/schema27.sql"),
    include_str!("sql/schema28.sql"),
    include_str!("sql/schema29.sql"),
    include_str!("sql/schema30.sql"),
    include_str!("sql/schema31.sql"),
    include_str!("sql/schema32.sql"),
    include_str!("sql/schema33.sql"),
    include_str!("sql/schema34.sql"),
    include_str!("sql/schema35.sql"),
    include_str!("sql/schema36.sql"),
    include_str!("sql/schema37.sql"),
];
