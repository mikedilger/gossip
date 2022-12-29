use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{EncryptedPrivateKey, PublicKey};
use serde::{Deserialize, Serialize};
use tracing::error;

pub const DEFAULT_FEED_CHUNK: u64 = 43200; // 12 hours
pub const DEFAULT_OVERLAP: u64 = 600; // 10 minutes
pub const DEFAULT_AUTOFOLLOW: bool = false;
pub const DEFAULT_VIEW_POSTS_REFERRED_TO: bool = true;
pub const DEFAULT_VIEW_POSTS_REFERRING_TO: bool = false;
pub const DEFAULT_VIEW_THREADED: bool = true;
pub const DEFAULT_NUM_RELAYS_PER_PERSON: u8 = 4;
pub const DEFAULT_MAX_RELAYS: u8 = 15;
pub const DEFAULT_MAX_FPS: u32 = 30;
pub const DEFAULT_FEED_RECOMPUTE_INTERVAL_MS: u32 = 2000;
pub const DEFAULT_POW: u8 = 0;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    pub feed_chunk: u64,
    pub overlap: u64,
    pub autofollow: bool,
    pub view_posts_referred_to: bool,
    pub view_posts_referring_to: bool,
    pub view_threaded: bool,
    pub num_relays_per_person: u8,
    pub max_relays: u8,
    pub public_key: Option<PublicKey>,
    pub encrypted_private_key: Option<EncryptedPrivateKey>,
    pub max_fps: u32,
    pub feed_recompute_interval_ms: u32,
    pub pow: u8,
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
            public_key: None,
            encrypted_private_key: None,
            max_fps: DEFAULT_MAX_FPS,
            feed_recompute_interval_ms: DEFAULT_FEED_RECOMPUTE_INTERVAL_MS,
            pow: DEFAULT_POW,
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
                "encrypted_private_key" => {
                    settings.encrypted_private_key = Some(EncryptedPrivateKey(row.1))
                }
                "public_key" => {
                    settings.public_key = match PublicKey::try_from_hex_string(&row.1) {
                        Ok(pk) => Some(pk),
                        Err(e) => {
                            error!("Public key in database is invalid or corrupt: {}", e);
                            None
                        }
                    }
                }
                "max_fps" => settings.max_fps = row.1.parse::<u32>().unwrap_or(DEFAULT_MAX_FPS),
                "feed_recompute_interval_ms" => {
                    settings.feed_recompute_interval_ms = row
                        .1
                        .parse::<u32>()
                        .unwrap_or(DEFAULT_FEED_RECOMPUTE_INTERVAL_MS)
                }
                "pow" => settings.pow = row.1.parse::<u8>().unwrap_or(DEFAULT_POW),
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
                                   ('view_threaded', ?),('num_relays_per_person', ?),\
                                   ('max_relays', ?),('max_fps', ?),('feed_recompute_interval_ms', ?),\
                                   ('pow', ?)",
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
            self.max_fps,
            self.feed_recompute_interval_ms,
            self.pow,
        ))?;

        // Save private key identity
        if let Some(ref epk) = self.encrypted_private_key {
            let mut stmt = db.prepare(
                "REPLACE INTO SETTINGS (key, value) VALUES ('encrypted_private_key', ?)",
            )?;
            stmt.execute((&epk.0,))?;
        } else {
            // Otherwise delete any such setting
            let mut stmt = db.prepare("DELETE FROM settings WHERE key='encrypted_private_key'")?;
            stmt.execute(())?;
        }

        // Save public key
        if let Some(ref pk) = self.public_key {
            let mut stmt =
                db.prepare("REPLACE INTO SETTINGS (key, value) VALUES ('public_key', ?)")?;
            stmt.execute((pk.as_hex_string(),))?;
        } else {
            // Otherwise delete any such setting
            let mut stmt = db.prepare("DELETE FROM settings WHERE key='public_key'")?;
            stmt.execute(())?;
        }
        Ok(())
    }
}
