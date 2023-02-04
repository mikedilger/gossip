use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{EncryptedPrivateKey, PublicKey};
use rusqlite::params;
use serde::{Deserialize, Serialize};

pub const DEFAULT_FEED_CHUNK: u64 = 60 * 60 * 12; // 12 hours
pub const DEFAULT_REPLIES_CHUNK: u64 = 60 * 60 * 24 * 7; // 1 week
pub const DEFAULT_OVERLAP: u64 = 300; // 5 minutes
pub const DEFAULT_NUM_RELAYS_PER_PERSON: u8 = 3;
pub const DEFAULT_MAX_RELAYS: u8 = 15;
pub const DEFAULT_MAX_FPS: u32 = 15;
pub const DEFAULT_FEED_RECOMPUTE_INTERVAL_MS: u32 = 3500;
pub const DEFAULT_POW: u8 = 0;
pub const DEFAULT_OFFLINE: bool = false;
pub const DEFAULT_LIGHT_MODE: bool = true; // true = light false = dark
pub const DEFAULT_SET_CLIENT_TAG: bool = false;
pub const DEFAULT_SET_USER_AGENT: bool = false;
pub const DEFAULT_OVERRIDE_DPI: Option<u32> = None;
pub const DEFAULT_REACTIONS: bool = true;
pub const DEFAULT_REPOSTS: bool = true;
pub const DEFAULT_LOAD_AVATARS: bool = true;
pub const DEFAULT_CHECK_NIP05: bool = true;
pub const DEFAULT_DIRECT_REPLIES_ONLY: bool = true;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    pub feed_chunk: u64,
    pub replies_chunk: u64,
    pub overlap: u64,
    pub num_relays_per_person: u8,
    pub max_relays: u8,
    pub public_key: Option<PublicKey>,
    pub encrypted_private_key: Option<EncryptedPrivateKey>,
    pub max_fps: u32,
    pub feed_recompute_interval_ms: u32,
    pub pow: u8,
    pub offline: bool,
    pub light_mode: bool,
    pub set_client_tag: bool,
    pub set_user_agent: bool,
    pub override_dpi: Option<u32>,
    pub reactions: bool,
    pub reposts: bool,
    pub load_avatars: bool,
    pub check_nip05: bool,
    pub direct_replies_only: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            feed_chunk: DEFAULT_FEED_CHUNK,
            replies_chunk: DEFAULT_REPLIES_CHUNK,
            overlap: DEFAULT_OVERLAP,
            num_relays_per_person: DEFAULT_NUM_RELAYS_PER_PERSON,
            max_relays: DEFAULT_MAX_RELAYS,
            public_key: None,
            encrypted_private_key: None,
            max_fps: DEFAULT_MAX_FPS,
            feed_recompute_interval_ms: DEFAULT_FEED_RECOMPUTE_INTERVAL_MS,
            pow: DEFAULT_POW,
            offline: DEFAULT_OFFLINE,
            light_mode: DEFAULT_LIGHT_MODE,
            set_client_tag: DEFAULT_SET_CLIENT_TAG,
            set_user_agent: DEFAULT_SET_USER_AGENT,
            override_dpi: DEFAULT_OVERRIDE_DPI,
            reactions: DEFAULT_REACTIONS,
            reposts: DEFAULT_REPOSTS,
            load_avatars: DEFAULT_LOAD_AVATARS,
            check_nip05: DEFAULT_CHECK_NIP05,
            direct_replies_only: DEFAULT_DIRECT_REPLIES_ONLY,
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
                "replies_chunk" => {
                    settings.replies_chunk = row.1.parse::<u64>().unwrap_or(DEFAULT_REPLIES_CHUNK)
                }
                "overlap" => settings.overlap = row.1.parse::<u64>().unwrap_or(DEFAULT_OVERLAP),
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
                            tracing::error!("Public key in database is invalid or corrupt: {}", e);
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
                "offline" => settings.offline = numstr_to_bool(row.1),
                "light_mode" => settings.light_mode = numstr_to_bool(row.1),
                "set_client_tag" => settings.set_client_tag = numstr_to_bool(row.1),
                "set_user_agent" => settings.set_user_agent = numstr_to_bool(row.1),
                "override_dpi" => {
                    if row.1.is_empty() {
                        settings.override_dpi = DEFAULT_OVERRIDE_DPI;
                    } else {
                        settings.override_dpi = match row.1.parse::<u32>() {
                            Ok(number) => Some(number),
                            _ => DEFAULT_OVERRIDE_DPI,
                        };
                    }
                }
                "reactions" => settings.reactions = numstr_to_bool(row.1),
                "reposts" => settings.reposts = numstr_to_bool(row.1),
                "load_avatars" => settings.load_avatars = numstr_to_bool(row.1),
                "check_nip05" => settings.check_nip05 = numstr_to_bool(row.1),
                "direct_replies_only" => settings.direct_replies_only = numstr_to_bool(row.1),
                _ => {}
            }
        }

        Ok(settings)
    }

    pub async fn save(&self) -> Result<(), Error> {
        let maybe_db = GLOBALS.db.lock().await;
        let db = maybe_db.as_ref().unwrap();

        let bool_to_numstr = |b: bool| -> &str {
            if b {
                "1"
            } else {
                "0"
            }
        };

        let mut stmt = db.prepare(
            "REPLACE INTO settings (key, value) VALUES \
             ('feed_chunk', ?),\
             ('replies_chunk', ?),\
             ('overlap', ?),\
             ('num_relays_per_person', ?),\
             ('max_relays', ?),\
             ('max_fps', ?),\
             ('feed_recompute_interval_ms', ?),\
             ('pow', ?),\
             ('offline', ?),\
             ('light_mode', ?),\
             ('set_client_tag', ?),\
             ('set_user_agent', ?),\
             ('reactions', ?),\
             ('reposts', ?),\
             ('load_avatars', ?),\
             ('check_nip05', ?),\
             ('direct_replies_only', ?)",
        )?;
        stmt.execute(params![
            self.feed_chunk,
            self.replies_chunk,
            self.overlap,
            self.num_relays_per_person,
            self.max_relays,
            self.max_fps,
            self.feed_recompute_interval_ms,
            self.pow,
            bool_to_numstr(self.offline),
            bool_to_numstr(self.light_mode),
            bool_to_numstr(self.set_client_tag),
            bool_to_numstr(self.set_user_agent),
            bool_to_numstr(self.reactions),
            bool_to_numstr(self.reposts),
            bool_to_numstr(self.load_avatars),
            bool_to_numstr(self.check_nip05),
            bool_to_numstr(self.direct_replies_only),
        ])?;

        // Save override dpi
        if let Some(ref dpi) = self.override_dpi {
            let mut stmt =
                db.prepare("REPLACE INTO SETTINGS (key, value) VALUES ('override_dpi', ?)")?;
            stmt.execute((&dpi,))?;
        } else {
            // Otherwise delete any such setting
            let mut stmt = db.prepare("DELETE FROM settings WHERE key='override_dpi'")?;
            stmt.execute(())?;
        }

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
