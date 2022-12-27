use crate::error::Error;
use crate::globals::GLOBALS;
use serde::{Deserialize, Serialize};

pub const DEFAULT_FEED_CHUNK: u64 = 43200; // 12 hours
pub const DEFAULT_OVERLAP: u64 = 600; // 10 minutes
pub const DEFAULT_AUTOFOLLOW: bool = false;
pub const DEFAULT_VIEW_POSTS_REFERRED_TO: bool = true;
pub const DEFAULT_VIEW_POSTS_REFERRING_TO: bool = false;
pub const DEFAULT_VIEW_THREADED: bool = true;
pub const DEFAULT_NUM_RELAYS_PER_PERSON: u8 = 2;
pub const DEFAULT_MAX_RELAYS: u8 = 15;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    // user_private_key
    // version
    pub feed_chunk: u64,
    pub overlap: u64,
    pub autofollow: bool,
    pub view_posts_referred_to: bool,
    pub view_posts_referring_to: bool,
    pub view_threaded: bool,
    pub num_relays_per_person: u8,
    pub max_relays: u8,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            feed_chunk: DEFAULT_FEED_CHUNK,
            overlap: DEFAULT_OVERLAP,
            autofollow: DEFAULT_AUTOFOLLOW,
            view_posts_referred_to: DEFAULT_VIEW_POSTS_REFERRED_TO,
            view_posts_referring_to: DEFAULT_VIEW_POSTS_REFERRING_TO,
            view_threaded: DEFAULT_VIEW_THREADED,
            num_relays_per_person: DEFAULT_NUM_RELAYS_PER_PERSON,
            max_relays: DEFAULT_MAX_RELAYS,
        }
    }
}

impl Settings {
    pub fn blocking_load() -> Result<Settings, Error> {
        let mut settings = Settings::default();

        let maybe_db = GLOBALS.db.blocking_lock();
        let db = maybe_db.as_ref().unwrap();

        let mut stmt = db.prepare("SELECT key, value FROM settings")?;

        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;

        let numstr_to_bool = |s: String| -> bool { &s == "1" };

        for row in rows {
            let row: (String, String) = row?;
            match &*row.0 {
                "feed_chunk" => {
                    settings.feed_chunk = row.1.parse::<u64>().unwrap_or(DEFAULT_FEED_CHUNK)
                }
                "overlap" => settings.overlap = row.1.parse::<u64>().unwrap_or(DEFAULT_OVERLAP),
                "autofollow" => settings.autofollow = numstr_to_bool(row.1),
                "view_posts_referred_to" => settings.view_posts_referred_to = numstr_to_bool(row.1),
                "view_posts_referring_to" => {
                    settings.view_posts_referring_to = numstr_to_bool(row.1)
                }
                "view_threaded" => settings.view_threaded = numstr_to_bool(row.1),
                "num_relays_per_person" => {
                    settings.num_relays_per_person =
                        row.1.parse::<u8>().unwrap_or(DEFAULT_NUM_RELAYS_PER_PERSON)
                }
                "max_relays" => {
                    settings.max_relays = row.1.parse::<u8>().unwrap_or(DEFAULT_MAX_RELAYS)
                }
                _ => {}
            }
        }

        Ok(settings)
    }

    pub async fn save(&self) -> Result<(), Error> {
        let maybe_db = GLOBALS.db.lock().await;
        let db = maybe_db.as_ref().unwrap();

        let mut stmt = db.prepare(
            "REPLACE INTO settings (key, value) VALUES \
                                   ('feed_chunk', ?),('overlap', ?),('autofollow', ?),\
                                   ('view_posts_referred_to', ?),('view_posts_referring_to', ?),\
                                   ('view_threaded', ?),('num_relays_per_person', ?), \
                                   ('max_relays', ?)",
        )?;
        stmt.execute((
            self.feed_chunk,
            self.overlap,
            if self.autofollow { "1" } else { "0" },
            if self.view_posts_referred_to {
                "1"
            } else {
                "0"
            },
            if self.view_posts_referring_to {
                "1"
            } else {
                "0"
            },
            if self.view_threaded { "1" } else { "0" },
            self.num_relays_per_person,
            self.max_relays,
        ))?;

        Ok(())
    }
}
