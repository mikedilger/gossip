mod legacy;
use super::Storage;
use crate::db::DbRelay;
use crate::error::Error;
use crate::settings::Settings;
use crate::ui::ThemeVariant;
use nostr_types::{EncryptedPrivateKey, Id, PublicKey, RelayUrl, Unixtime};
use rusqlite::Connection;

impl Storage {
    pub(super) fn import(&self) -> Result<(), Error> {
        tracing::info!("Importing SQLITE data into LMDB...");

        // Progress the legacy database to the endpoint first
        let mut db = legacy::init_database()?;
        legacy::setup_database(&mut db)?;
        tracing::info!("LDMB: setup");

        // local settings
        import_local_settings(&db, |epk: Option<EncryptedPrivateKey>, lcle: i64| {
            self.write_encrypted_private_key(&epk)?;
            self.write_last_contact_list_edit(lcle)
        })?;

        // old table "settings"
        // Copy settings (including local_settings)
        import_settings(&db, |settings: &Settings| self.write_settings(settings))?;

        // old table "event_relay"
        // Copy events_seen
        import_event_seen_on_relay(&db, |id: String, url: String, seen: u64| {
            let id = Id::try_from_hex_string(&id)?;
            let relay_url = RelayUrl(url);
            let time = Unixtime(seen as i64);
            self.add_event_seen_on_relay(id, &relay_url, time)
        })?;

        // old table "event_flags"
        // Copy event_flags
        import_event_flags(&db, |id: Id, viewed: bool| {
            if viewed {
                self.mark_event_viewed(id)
            } else {
                Ok(())
            }
        })?;

        // old table "event_hashtag"
        // Copy event_hashtags
        import_hashtags(&db, |hashtag: String, event: String| {
            let id = Id::try_from_hex_string(&event)?;
            self.add_hashtag(&hashtag, id)
        })?;

        // old table "relay"
        // Copy relays
        import_relays(&db, |dbrelay: &DbRelay| self.write_relay(dbrelay))?;

        // Mark migration level
        // TBD: self.write_migration_level(0)?;

        tracing::info!("Importing SQLITE data into LMDB: Done.");

        Ok(())
    }
}

fn import_local_settings<F>(db: &Connection, mut f: F) -> Result<(), Error>
where
    F: FnMut(Option<EncryptedPrivateKey>, i64) -> Result<(), Error>,
{
    // These are the only local settings we need to keep
    let sql = "SELECT encrypted_private_key, last_contact_list_edit FROM local_settings";
    let mut stmt = db.prepare(sql)?;
    let mut rows = stmt.raw_query();
    if let Some(row) = rows.next()? {
        let epk: Option<String> = row.get(0)?;
        let lcle: i64 = row.get(1)?;
        f(epk.map(EncryptedPrivateKey), lcle)?;
    }
    Ok(())
}

fn import_settings<F>(db: &Connection, mut f: F) -> Result<(), Error>
where
    F: FnMut(&Settings) -> Result<(), Error>,
{
    let numstr_to_bool = |s: String| -> bool { &s == "1" };

    let sql = "SELECT key, value FROM settings ORDER BY key";
    let mut stmt = db.prepare(sql)?;
    let mut rows = stmt.raw_query();
    let mut settings = Settings::default();
    while let Some(row) = rows.next()? {
        let key: String = row.get(0)?;
        let value: String = row.get(1)?;
        match &*key {
            "feed_chunk" => {
                if let Ok(x) = value.parse::<u64>() {
                    settings.feed_chunk = x;
                }
            }
            "replies_chunk" => {
                if let Ok(x) = value.parse::<u64>() {
                    settings.replies_chunk = x;
                }
            }
            "overlap" => {
                if let Ok(x) = value.parse::<u64>() {
                    settings.overlap = x;
                }
            }
            "num_relays_per_person" => {
                if let Ok(x) = value.parse::<u8>() {
                    settings.num_relays_per_person = x;
                }
            }
            "max_relays" => {
                if let Ok(x) = value.parse::<u8>() {
                    settings.max_relays = x;
                }
            }
            "public_key" => {
                settings.public_key = match PublicKey::try_from_hex_string(&value) {
                    Ok(pk) => Some(pk),
                    Err(e) => {
                        tracing::error!("Public key in database is invalid or corrupt: {}", e);
                        None
                    }
                }
            }
            "max_fps" => {
                if let Ok(x) = value.parse::<u32>() {
                    settings.max_fps = x;
                }
            }
            "recompute_feed_periodically" => {
                settings.recompute_feed_periodically = numstr_to_bool(value)
            }
            "feed_recompute_interval_ms" => {
                if let Ok(x) = value.parse::<u32>() {
                    settings.feed_recompute_interval_ms = x;
                }
            }
            "pow" => {
                if let Ok(x) = value.parse::<u8>() {
                    settings.pow = x;
                }
            }
            "offline" => settings.offline = numstr_to_bool(value),
            "dark_mode" => settings.theme.dark_mode = numstr_to_bool(value),
            "follow_os_dark_mode" => settings.theme.follow_os_dark_mode = numstr_to_bool(value),
            "theme" => {
                for theme_variant in ThemeVariant::all() {
                    if &*value == theme_variant.name() {
                        settings.theme.variant = *theme_variant;
                        break;
                    }
                }
            }
            "set_client_tag" => settings.set_client_tag = numstr_to_bool(value),
            "set_user_agent" => settings.set_user_agent = numstr_to_bool(value),
            "override_dpi" => {
                if value.is_empty() {
                    settings.override_dpi = None;
                } else if let Ok(x) = value.parse::<u32>() {
                    settings.override_dpi = Some(x);
                }
            }
            "reactions" => settings.reactions = numstr_to_bool(value),
            "reposts" => settings.reposts = numstr_to_bool(value),
            "show_long_form" => settings.show_long_form = numstr_to_bool(value),
            "show_mentions" => settings.show_mentions = numstr_to_bool(value),
            "show_media" => settings.show_media = numstr_to_bool(value),
            "load_avatars" => settings.load_avatars = numstr_to_bool(value),
            "load_media" => settings.load_media = numstr_to_bool(value),
            "check_nip05" => settings.check_nip05 = numstr_to_bool(value),
            "direct_messages" => settings.direct_messages = numstr_to_bool(value),
            "automatically_fetch_metadata" => {
                settings.automatically_fetch_metadata = numstr_to_bool(value)
            }
            "delegatee_tag" => settings.delegatee_tag = value,
            "highlight_unread_events" => settings.highlight_unread_events = numstr_to_bool(value),
            "posting_area_at_top" => settings.posting_area_at_top = numstr_to_bool(value),
            "enable_zap_receipts" => settings.enable_zap_receipts = numstr_to_bool(value),
            _ => {}
        }
    }

    f(&settings)?;

    Ok(())
}

fn import_event_seen_on_relay<F>(db: &Connection, mut f: F) -> Result<(), Error>
where
    F: FnMut(String, String, u64) -> Result<(), Error>,
{
    let sql = "SELECT event, relay, when_seen FROM event_relay ORDER BY event, relay";
    let mut stmt = db.prepare(sql)?;
    let mut rows = stmt.raw_query();
    while let Some(row) = rows.next()? {
        let event: String = row.get(0)?;
        let relay: String = row.get(1)?;
        let seen: u64 = row.get(2)?;
        f(event, relay, seen)?;
    }
    Ok(())
}

fn import_event_flags<F>(db: &Connection, mut f: F) -> Result<(), Error>
where
    F: FnMut(Id, bool) -> Result<(), Error>,
{
    let sql = "SELECT event, viewed FROM event_flags ORDER BY event";
    let mut stmt = db.prepare(sql)?;
    let mut rows = stmt.raw_query();
    while let Some(row) = rows.next()? {
        let idstr: String = row.get(0)?;
        let viewed: bool = row.get(1)?;
        let id: Id = match Id::try_from_hex_string(&idstr) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("{}", e);
                // don't process the broken one
                continue;
            }
        };
        f(id, viewed)?;
    }
    Ok(())
}

fn import_hashtags<F>(db: &Connection, mut f: F) -> Result<(), Error>
where
    F: FnMut(String, String) -> Result<(), Error>,
{
    let sql = "SELECT hashtag, event FROM event_hashtag ORDER BY hashtag, event";
    let mut stmt = db.prepare(sql)?;
    let mut rows = stmt.raw_query();
    while let Some(row) = rows.next()? {
        let hashtag: String = row.get(0)?;
        let event: String = row.get(1)?;
        f(hashtag, event)?;
    }
    Ok(())
}

fn import_relays<F>(db: &Connection, mut f: F) -> Result<(), Error>
where
    F: FnMut(&DbRelay) -> Result<(), Error>,
{
    let sql = "SELECT url, success_count, failure_count, last_connected_at, \
               last_general_eose_at, rank, hidden, usage_bits, \
               nip11, last_attempt_nip11 FROM relay ORDER BY url"
        .to_owned();
    let mut stmt = db.prepare(&sql)?;
    let mut rows = stmt.raw_query();
    while let Some(row) = rows.next()? {
        let urlstring: String = row.get(0)?;
        let nip11: Option<String> = row.get(8)?;
        if let Ok(url) = RelayUrl::try_from_str(&urlstring) {
            let dbrelay = DbRelay {
                url,
                success_count: row.get(1)?,
                failure_count: row.get(2)?,
                last_connected_at: row.get(3)?,
                last_general_eose_at: row.get(4)?,
                rank: row.get(5)?,
                hidden: row.get(6)?,
                usage_bits: row.get(7)?,
                nip11: match nip11 {
                    None => None,
                    Some(s) => serde_json::from_str(&s)?,
                },
                last_attempt_nip11: row.get(9)?,
            };
            f(&dbrelay)?;
        }
    }
    Ok(())
}
