use crate::error::Error;
use crate::globals::GLOBALS;
use crate::ui::{Theme, ThemeVariant};
use nostr_types::PublicKey;
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
pub const DEFAULT_THEME: Theme = Theme {
    variant: ThemeVariant::Default,
    dark_mode: false,
};
pub const DEFAULT_SET_CLIENT_TAG: bool = false;
pub const DEFAULT_SET_USER_AGENT: bool = false;
pub const DEFAULT_OVERRIDE_DPI: Option<u32> = None;
pub const DEFAULT_REACTIONS: bool = true;
pub const DEFAULT_REPOSTS: bool = true;
pub const DEFAULT_SHOW_FIRST_MENTION: bool = true;
pub const DEFAULT_LOAD_AVATARS: bool = true;
pub const DEFAULT_CHECK_NIP05: bool = true;
pub const DEFAULT_DIRECT_MESSAGES: bool = true;
pub const DEFAULT_AUTOMATICALLY_FETCH_METADATA: bool = true;
pub const DEFAULT_HIGHLIGHT_UNREAD_EVENTS: bool = true;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub feed_chunk: u64,
    pub replies_chunk: u64,
    pub overlap: u64,
    pub num_relays_per_person: u8,
    pub max_relays: u8,
    pub public_key: Option<PublicKey>,
    pub max_fps: u32,
    pub feed_recompute_interval_ms: u32,
    pub pow: u8,
    pub offline: bool,
    pub theme: Theme,
    pub set_client_tag: bool,
    pub set_user_agent: bool,
    pub override_dpi: Option<u32>,
    pub reactions: bool,
    pub reposts: bool,
    pub show_first_mention: bool,
    pub load_avatars: bool,
    pub check_nip05: bool,
    pub direct_messages: bool,
    pub automatically_fetch_metadata: bool,
    pub delegatee_tag: String,
    pub highlight_unread_events: bool,
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
            max_fps: DEFAULT_MAX_FPS,
            feed_recompute_interval_ms: DEFAULT_FEED_RECOMPUTE_INTERVAL_MS,
            pow: DEFAULT_POW,
            offline: DEFAULT_OFFLINE,
            theme: DEFAULT_THEME,
            set_client_tag: DEFAULT_SET_CLIENT_TAG,
            set_user_agent: DEFAULT_SET_USER_AGENT,
            override_dpi: DEFAULT_OVERRIDE_DPI,
            reactions: DEFAULT_REACTIONS,
            reposts: DEFAULT_REPOSTS,
            show_first_mention: DEFAULT_SHOW_FIRST_MENTION,
            load_avatars: DEFAULT_LOAD_AVATARS,
            check_nip05: DEFAULT_CHECK_NIP05,
            direct_messages: DEFAULT_DIRECT_MESSAGES,
            automatically_fetch_metadata: DEFAULT_AUTOMATICALLY_FETCH_METADATA,
            delegatee_tag: String::new(),
            highlight_unread_events: DEFAULT_HIGHLIGHT_UNREAD_EVENTS,
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
                "dark_mode" => settings.theme.dark_mode = numstr_to_bool(row.1),
                "theme" => {
                    for theme_variant in ThemeVariant::all() {
                        if &*row.1 == theme_variant.name() {
                            settings.theme.variant = *theme_variant;
                            break;
                        }
                    }
                }
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
                "show_first_mention" => settings.show_first_mention = numstr_to_bool(row.1),
                "load_avatars" => settings.load_avatars = numstr_to_bool(row.1),
                "check_nip05" => settings.check_nip05 = numstr_to_bool(row.1),
                "direct_messages" => settings.direct_messages = numstr_to_bool(row.1),
                "automatically_fetch_metadata" => {
                    settings.automatically_fetch_metadata = numstr_to_bool(row.1)
                }
                "delegatee_tag" => settings.delegatee_tag = row.1,
                "highlight_unread_events" => {
                    settings.highlight_unread_events = numstr_to_bool(row.1)
                }
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
             ('dark_mode', ?),\
             ('theme', ?),\
             ('set_client_tag', ?),\
             ('set_user_agent', ?),\
             ('reactions', ?),\
             ('reposts', ?),\
             ('show_first_mention', ?),\
             ('load_avatars', ?),\
             ('check_nip05', ?),\
             ('direct_messages', ?),\
             ('automatically_fetch_metadata', ?),\
             ('delegatee_tag', ?),\
             ('highlight_unread_events', ?)",
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
            bool_to_numstr(self.theme.dark_mode),
            self.theme.variant.name(),
            bool_to_numstr(self.set_client_tag),
            bool_to_numstr(self.set_user_agent),
            bool_to_numstr(self.reactions),
            bool_to_numstr(self.reposts),
            bool_to_numstr(self.show_first_mention),
            bool_to_numstr(self.load_avatars),
            bool_to_numstr(self.check_nip05),
            bool_to_numstr(self.direct_messages),
            bool_to_numstr(self.automatically_fetch_metadata),
            self.delegatee_tag,
            bool_to_numstr(self.highlight_unread_events),
        ])?;

        // Settings which are Options should not even exist when None.  We don't accept null valued
        // settings.

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
