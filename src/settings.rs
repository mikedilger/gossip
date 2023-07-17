use crate::error::Error;
use crate::globals::GLOBALS;
use crate::ui::{Theme, ThemeVariant};
use nostr_types::{EventKind, PublicKey};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub feed_chunk: u64,
    pub replies_chunk: u64,
    pub overlap: u64,
    pub num_relays_per_person: u8,
    pub max_relays: u8,
    pub public_key: Option<PublicKey>,
    pub max_fps: u32,
    pub recompute_feed_periodically: bool,
    pub feed_recompute_interval_ms: u32,
    pub pow: u8,
    pub offline: bool,
    pub theme: Theme,
    pub set_client_tag: bool,
    pub set_user_agent: bool,
    pub override_dpi: Option<u32>,
    pub reactions: bool,
    pub reposts: bool,
    pub show_long_form: bool,
    pub show_mentions: bool,
    pub show_media: bool,
    pub load_avatars: bool,
    pub load_media: bool,
    pub check_nip05: bool,
    pub direct_messages: bool,
    pub automatically_fetch_metadata: bool,
    pub delegatee_tag: String,
    pub highlight_unread_events: bool,
    pub posting_area_at_top: bool,
    pub enable_zap_receipts: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            feed_chunk: 60 * 60 * 12,        // 12 hours
            replies_chunk: 60 * 60 * 24 * 7, // 1 week
            overlap: 300,                    // 5 minutes
            num_relays_per_person: 2,
            max_relays: 50,
            public_key: None,
            max_fps: 12,
            recompute_feed_periodically: true,
            feed_recompute_interval_ms: 2000,
            pow: 0,
            offline: false,
            theme: Theme {
                variant: ThemeVariant::Default,
                dark_mode: false,
                follow_os_dark_mode: false,
            },
            set_client_tag: false,
            set_user_agent: false,
            override_dpi: None,
            reactions: true,
            reposts: true,
            show_long_form: false,
            show_mentions: true,
            show_media: true,
            load_avatars: true,
            load_media: true,
            check_nip05: true,
            direct_messages: true,
            automatically_fetch_metadata: true,
            delegatee_tag: String::new(),
            highlight_unread_events: true,
            posting_area_at_top: true,
            enable_zap_receipts: true,
        }
    }
}

impl Settings {
    pub fn blocking_load() -> Result<Settings, Error> {
        let mut settings = Settings::default();

        let db = GLOBALS.db.blocking_lock();

        let mut stmt = db.prepare("SELECT key, value FROM settings")?;

        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;

        let numstr_to_bool = |s: String| -> bool { &s == "1" };

        for row in rows {
            let row: (String, String) = row?;
            match &*row.0 {
                "feed_chunk" => {
                    if let Ok(x) = row.1.parse::<u64>() {
                        settings.feed_chunk = x;
                    }
                }
                "replies_chunk" => {
                    if let Ok(x) = row.1.parse::<u64>() {
                        settings.replies_chunk = x;
                    }
                }
                "overlap" => {
                    if let Ok(x) = row.1.parse::<u64>() {
                        settings.overlap = x;
                    }
                }
                "num_relays_per_person" => {
                    if let Ok(x) = row.1.parse::<u8>() {
                        settings.num_relays_per_person = x;
                    }
                }
                "max_relays" => {
                    if let Ok(x) = row.1.parse::<u8>() {
                        settings.max_relays = x;
                    }
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
                "max_fps" => {
                    if let Ok(x) = row.1.parse::<u32>() {
                        settings.max_fps = x;
                    }
                }
                "recompute_feed_periodically" => {
                    settings.recompute_feed_periodically = numstr_to_bool(row.1)
                }
                "feed_recompute_interval_ms" => {
                    if let Ok(x) = row.1.parse::<u32>() {
                        settings.feed_recompute_interval_ms = x;
                    }
                }
                "pow" => {
                    if let Ok(x) = row.1.parse::<u8>() {
                        settings.pow = x;
                    }
                }
                "offline" => settings.offline = numstr_to_bool(row.1),
                "dark_mode" => settings.theme.dark_mode = numstr_to_bool(row.1),
                "follow_os_dark_mode" => settings.theme.follow_os_dark_mode = numstr_to_bool(row.1),
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
                        settings.override_dpi = None;
                    } else if let Ok(x) = row.1.parse::<u32>() {
                        settings.override_dpi = Some(x);
                    }
                }
                "reactions" => settings.reactions = numstr_to_bool(row.1),
                "reposts" => settings.reposts = numstr_to_bool(row.1),
                "show_long_form" => settings.show_long_form = numstr_to_bool(row.1),
                "show_mentions" => settings.show_mentions = numstr_to_bool(row.1),
                "show_media" => settings.show_media = numstr_to_bool(row.1),
                "load_avatars" => settings.load_avatars = numstr_to_bool(row.1),
                "load_media" => settings.load_media = numstr_to_bool(row.1),
                "check_nip05" => settings.check_nip05 = numstr_to_bool(row.1),
                "direct_messages" => settings.direct_messages = numstr_to_bool(row.1),
                "automatically_fetch_metadata" => {
                    settings.automatically_fetch_metadata = numstr_to_bool(row.1)
                }
                "delegatee_tag" => settings.delegatee_tag = row.1,
                "highlight_unread_events" => {
                    settings.highlight_unread_events = numstr_to_bool(row.1)
                }
                "posting_area_at_top" => settings.posting_area_at_top = numstr_to_bool(row.1),
                "enable_zap_receipts" => settings.enable_zap_receipts = numstr_to_bool(row.1),
                _ => {}
            }
        }

        Ok(settings)
    }

    pub async fn save(&self) -> Result<(), Error> {
        let db = GLOBALS.db.lock().await;

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
             ('recompute_feed_periodically', ?),\
             ('feed_recompute_interval_ms', ?),\
             ('pow', ?),\
             ('offline', ?),\
             ('dark_mode', ?),\
             ('follow_os_dark_mode', ?),\
             ('theme', ?),\
             ('set_client_tag', ?),\
             ('set_user_agent', ?),\
             ('reactions', ?),\
             ('reposts', ?),\
             ('show_long_form', ?),\
             ('show_mentions', ?),\
             ('show_media', ?),\
             ('load_avatars', ?),\
             ('load_media', ?),\
             ('check_nip05', ?),\
             ('direct_messages', ?),\
             ('automatically_fetch_metadata', ?),\
             ('delegatee_tag', ?),\
             ('highlight_unread_events', ?),\
             ('posting_area_at_top', ?),\
             ('enable_zap_receipts', ?)",
        )?;
        stmt.execute(params![
            self.feed_chunk,
            self.replies_chunk,
            self.overlap,
            self.num_relays_per_person,
            self.max_relays,
            self.max_fps,
            self.recompute_feed_periodically,
            self.feed_recompute_interval_ms,
            self.pow,
            bool_to_numstr(self.offline),
            bool_to_numstr(self.theme.dark_mode),
            bool_to_numstr(self.theme.follow_os_dark_mode),
            self.theme.variant.name(),
            bool_to_numstr(self.set_client_tag),
            bool_to_numstr(self.set_user_agent),
            bool_to_numstr(self.reactions),
            bool_to_numstr(self.reposts),
            bool_to_numstr(self.show_long_form),
            bool_to_numstr(self.show_mentions),
            bool_to_numstr(self.show_media),
            bool_to_numstr(self.load_avatars),
            bool_to_numstr(self.load_media),
            bool_to_numstr(self.check_nip05),
            bool_to_numstr(self.direct_messages),
            bool_to_numstr(self.automatically_fetch_metadata),
            self.delegatee_tag,
            bool_to_numstr(self.highlight_unread_events),
            bool_to_numstr(self.posting_area_at_top),
            bool_to_numstr(self.enable_zap_receipts),
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

    pub fn enabled_event_kinds(&self) -> Vec<EventKind> {
        EventKind::iter()
            .filter(|k| {
                ((*k != EventKind::Reaction) || self.reactions)
                    && ((*k != EventKind::Repost) || self.reposts)
                    && ((*k != EventKind::LongFormContent) || self.show_long_form)
                    && ((*k != EventKind::EncryptedDirectMessage) || self.direct_messages)
                    && ((*k != EventKind::Zap) || self.enable_zap_receipts)
            })
            .collect()
    }

    pub fn feed_related_event_kinds(&self) -> Vec<EventKind> {
        self.enabled_event_kinds()
            .drain(..)
            .filter(|k| k.is_feed_related())
            .collect()
    }
}
