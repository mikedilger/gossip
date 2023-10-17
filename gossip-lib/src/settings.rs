use crate::error::Error;
use crate::globals::GLOBALS;
use crate::storage::Storage;
use nostr_types::PublicKey;
use paste::paste;
use serde::{Deserialize, Serialize};
use speedy::{Readable, Writable};

macro_rules! load_setting {
    ($field:ident) => {
        paste! {
            GLOBALS.storage.[<read_setting_ $field>]()
        }
    };
}

macro_rules! default_setting {
    ($field:ident) => {
        paste! {
            Storage::[<get_default_setting_ $field>]()
        }
    };
}

macro_rules! save_setting {
    ($field:ident, $slf:ident, $txn:ident) => {
        paste! {
            GLOBALS.storage.[<write_setting_ $field>](&$slf.$field, Some(&mut $txn))?;
        }
    };
}

/// Settings are stored in GLOBALS.storage individually. Usually we don't need them together
/// as an object. But the UI uses this to cache changes before committing them.
///
/// NOTE: It is recommended to NOT use this structure. Instead, just interact with each
/// setting key individually via `GLOBALS.storage`
#[derive(Clone, Debug, Serialize, Deserialize, Readable, Writable, PartialEq)]
pub struct Settings {
    // ID settings
    pub public_key: Option<PublicKey>,
    pub log_n: u8,

    // Network settings
    pub offline: bool,
    pub load_avatars: bool,
    pub load_media: bool,
    pub check_nip05: bool,
    pub automatically_fetch_metadata: bool,

    // Relay settings
    pub num_relays_per_person: u8,
    pub max_relays: u8,

    // Feed Settings
    pub feed_chunk: u64,
    pub replies_chunk: u64,
    pub person_feed_chunk: u64,
    pub overlap: u64,
    pub custom_person_list_names: [String; 10],

    // Event Selection
    pub reposts: bool,
    pub show_long_form: bool,
    pub show_mentions: bool,
    pub direct_messages: bool,
    pub future_allowance_secs: u64,

    // Event Content Settings
    pub hide_mutes_entirely: bool,
    pub reactions: bool,
    pub enable_zap_receipts: bool,
    pub show_media: bool,
    pub approve_content_warning: bool,
    pub show_deleted_events: bool,

    // Posting Settings
    pub pow: u8,
    pub set_client_tag: bool,
    pub set_user_agent: bool,
    pub delegatee_tag: String,

    // UI settings
    pub max_fps: u32,
    pub recompute_feed_periodically: bool,
    pub feed_recompute_interval_ms: u32,
    pub theme_variant: String,
    pub dark_mode: bool,
    pub follow_os_dark_mode: bool,
    pub override_dpi: Option<u32>,
    pub highlight_unread_events: bool,
    pub posting_area_at_top: bool,
    pub status_bar: bool,
    pub image_resize_algorithm: String,
    pub inertial_scrolling: bool,
    pub mouse_acceleration: f32,

    // Staletime settings
    pub relay_list_becomes_stale_hours: u64,
    pub metadata_becomes_stale_hours: u64,
    pub nip05_becomes_stale_if_valid_hours: u64,
    pub nip05_becomes_stale_if_invalid_minutes: u64,
    pub avatar_becomes_stale_hours: u64,
    pub media_becomes_stale_hours: u64,

    // Websocket settings
    pub max_websocket_message_size_kb: usize,
    pub max_websocket_frame_size_kb: usize,
    pub websocket_accept_unmasked_frames: bool,
    pub websocket_connect_timeout_sec: u64,
    pub websocket_ping_frequency_sec: u64,

    // HTTP settings
    pub fetcher_metadata_looptime_ms: u64,
    pub fetcher_looptime_ms: u64,
    pub fetcher_connect_timeout_sec: u64,
    pub fetcher_timeout_sec: u64,
    pub fetcher_max_requests_per_host: usize,
    pub fetcher_host_exclusion_on_low_error_secs: u64,
    pub fetcher_host_exclusion_on_med_error_secs: u64,
    pub fetcher_host_exclusion_on_high_error_secs: u64,
    pub nip11_lines_to_output_on_error: usize,

    // Database settings
    pub prune_period_days: u64,
    pub cache_prune_period_days: u64,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            public_key: default_setting!(public_key),
            log_n: default_setting!(log_n),
            offline: default_setting!(offline),
            load_avatars: default_setting!(load_avatars),
            load_media: default_setting!(load_media),
            check_nip05: default_setting!(check_nip05),
            automatically_fetch_metadata: default_setting!(automatically_fetch_metadata),
            num_relays_per_person: default_setting!(num_relays_per_person),
            max_relays: default_setting!(max_relays),
            feed_chunk: default_setting!(feed_chunk),
            replies_chunk: default_setting!(replies_chunk),
            person_feed_chunk: default_setting!(person_feed_chunk),
            overlap: default_setting!(overlap),
            custom_person_list_names: default_setting!(custom_person_list_names),
            reposts: default_setting!(reposts),
            show_long_form: default_setting!(show_long_form),
            show_mentions: default_setting!(show_mentions),
            direct_messages: default_setting!(direct_messages),
            future_allowance_secs: default_setting!(future_allowance_secs),
            hide_mutes_entirely: default_setting!(hide_mutes_entirely),
            reactions: default_setting!(reactions),
            enable_zap_receipts: default_setting!(enable_zap_receipts),
            show_media: default_setting!(show_media),
            approve_content_warning: default_setting!(approve_content_warning),
            show_deleted_events: default_setting!(show_deleted_events),
            pow: default_setting!(pow),
            set_client_tag: default_setting!(set_client_tag),
            set_user_agent: default_setting!(set_user_agent),
            delegatee_tag: default_setting!(delegatee_tag),
            max_fps: default_setting!(max_fps),
            recompute_feed_periodically: default_setting!(recompute_feed_periodically),
            feed_recompute_interval_ms: default_setting!(feed_recompute_interval_ms),
            theme_variant: default_setting!(theme_variant),
            dark_mode: default_setting!(dark_mode),
            follow_os_dark_mode: default_setting!(follow_os_dark_mode),
            override_dpi: default_setting!(override_dpi),
            highlight_unread_events: default_setting!(highlight_unread_events),
            posting_area_at_top: default_setting!(posting_area_at_top),
            status_bar: default_setting!(status_bar),
            image_resize_algorithm: default_setting!(image_resize_algorithm),
            inertial_scrolling: default_setting!(inertial_scrolling),
            mouse_acceleration: default_setting!(mouse_acceleration),
            relay_list_becomes_stale_hours: default_setting!(relay_list_becomes_stale_hours),
            metadata_becomes_stale_hours: default_setting!(metadata_becomes_stale_hours),
            nip05_becomes_stale_if_valid_hours: default_setting!(
                nip05_becomes_stale_if_valid_hours
            ),
            nip05_becomes_stale_if_invalid_minutes: default_setting!(
                nip05_becomes_stale_if_invalid_minutes
            ),
            avatar_becomes_stale_hours: default_setting!(avatar_becomes_stale_hours),
            media_becomes_stale_hours: default_setting!(media_becomes_stale_hours),
            max_websocket_message_size_kb: default_setting!(max_websocket_message_size_kb),
            max_websocket_frame_size_kb: default_setting!(max_websocket_frame_size_kb),
            websocket_accept_unmasked_frames: default_setting!(websocket_accept_unmasked_frames),
            websocket_connect_timeout_sec: default_setting!(websocket_connect_timeout_sec),
            websocket_ping_frequency_sec: default_setting!(websocket_ping_frequency_sec),
            fetcher_metadata_looptime_ms: default_setting!(fetcher_metadata_looptime_ms),
            fetcher_looptime_ms: default_setting!(fetcher_looptime_ms),
            fetcher_connect_timeout_sec: default_setting!(fetcher_connect_timeout_sec),
            fetcher_timeout_sec: default_setting!(fetcher_timeout_sec),
            fetcher_max_requests_per_host: default_setting!(fetcher_max_requests_per_host),
            fetcher_host_exclusion_on_low_error_secs: default_setting!(
                fetcher_host_exclusion_on_low_error_secs
            ),
            fetcher_host_exclusion_on_med_error_secs: default_setting!(
                fetcher_host_exclusion_on_med_error_secs
            ),
            fetcher_host_exclusion_on_high_error_secs: default_setting!(
                fetcher_host_exclusion_on_high_error_secs
            ),
            nip11_lines_to_output_on_error: default_setting!(nip11_lines_to_output_on_error),
            prune_period_days: default_setting!(prune_period_days),
            cache_prune_period_days: default_setting!(prune_period_days),
        }
    }
}

impl Settings {
    pub fn load() -> Settings {
        Settings {
            public_key: load_setting!(public_key),
            log_n: load_setting!(log_n),
            offline: load_setting!(offline),
            load_avatars: load_setting!(load_avatars),
            load_media: load_setting!(load_media),
            check_nip05: load_setting!(check_nip05),
            automatically_fetch_metadata: load_setting!(automatically_fetch_metadata),
            num_relays_per_person: load_setting!(num_relays_per_person),
            max_relays: load_setting!(max_relays),
            feed_chunk: load_setting!(feed_chunk),
            replies_chunk: load_setting!(replies_chunk),
            person_feed_chunk: load_setting!(person_feed_chunk),
            overlap: load_setting!(overlap),
            custom_person_list_names: load_setting!(custom_person_list_names),
            reposts: load_setting!(reposts),
            show_long_form: load_setting!(show_long_form),
            show_mentions: load_setting!(show_mentions),
            direct_messages: load_setting!(direct_messages),
            future_allowance_secs: load_setting!(future_allowance_secs),
            hide_mutes_entirely: load_setting!(hide_mutes_entirely),
            reactions: load_setting!(reactions),
            enable_zap_receipts: load_setting!(enable_zap_receipts),
            show_media: load_setting!(show_media),
            approve_content_warning: load_setting!(approve_content_warning),
            show_deleted_events: load_setting!(show_deleted_events),
            pow: load_setting!(pow),
            set_client_tag: load_setting!(set_client_tag),
            set_user_agent: load_setting!(set_user_agent),
            delegatee_tag: load_setting!(delegatee_tag),
            max_fps: load_setting!(max_fps),
            recompute_feed_periodically: load_setting!(recompute_feed_periodically),
            feed_recompute_interval_ms: load_setting!(feed_recompute_interval_ms),
            theme_variant: load_setting!(theme_variant),
            dark_mode: load_setting!(dark_mode),
            follow_os_dark_mode: load_setting!(follow_os_dark_mode),
            override_dpi: load_setting!(override_dpi),
            highlight_unread_events: load_setting!(highlight_unread_events),
            posting_area_at_top: load_setting!(posting_area_at_top),
            status_bar: load_setting!(status_bar),
            image_resize_algorithm: load_setting!(image_resize_algorithm),
            inertial_scrolling: load_setting!(inertial_scrolling),
            mouse_acceleration: load_setting!(mouse_acceleration),
            relay_list_becomes_stale_hours: load_setting!(relay_list_becomes_stale_hours),
            metadata_becomes_stale_hours: load_setting!(metadata_becomes_stale_hours),
            nip05_becomes_stale_if_valid_hours: load_setting!(nip05_becomes_stale_if_valid_hours),
            nip05_becomes_stale_if_invalid_minutes: load_setting!(
                nip05_becomes_stale_if_invalid_minutes
            ),
            avatar_becomes_stale_hours: load_setting!(avatar_becomes_stale_hours),
            media_becomes_stale_hours: load_setting!(media_becomes_stale_hours),
            max_websocket_message_size_kb: load_setting!(max_websocket_message_size_kb),
            max_websocket_frame_size_kb: load_setting!(max_websocket_frame_size_kb),
            websocket_accept_unmasked_frames: load_setting!(websocket_accept_unmasked_frames),
            websocket_connect_timeout_sec: load_setting!(websocket_connect_timeout_sec),
            websocket_ping_frequency_sec: load_setting!(websocket_ping_frequency_sec),
            fetcher_metadata_looptime_ms: load_setting!(fetcher_metadata_looptime_ms),
            fetcher_looptime_ms: load_setting!(fetcher_looptime_ms),
            fetcher_connect_timeout_sec: load_setting!(fetcher_connect_timeout_sec),
            fetcher_timeout_sec: load_setting!(fetcher_timeout_sec),
            fetcher_max_requests_per_host: load_setting!(fetcher_max_requests_per_host),
            fetcher_host_exclusion_on_low_error_secs: load_setting!(
                fetcher_host_exclusion_on_low_error_secs
            ),
            fetcher_host_exclusion_on_med_error_secs: load_setting!(
                fetcher_host_exclusion_on_med_error_secs
            ),
            fetcher_host_exclusion_on_high_error_secs: load_setting!(
                fetcher_host_exclusion_on_high_error_secs
            ),
            nip11_lines_to_output_on_error: load_setting!(nip11_lines_to_output_on_error),
            prune_period_days: load_setting!(prune_period_days),
            cache_prune_period_days: load_setting!(cache_prune_period_days),
        }
    }

    pub fn save(&self) -> Result<(), Error> {
        let mut txn = GLOBALS.storage.get_write_txn()?;
        save_setting!(public_key, self, txn);
        save_setting!(log_n, self, txn);
        save_setting!(offline, self, txn);
        save_setting!(load_avatars, self, txn);
        save_setting!(load_media, self, txn);
        save_setting!(check_nip05, self, txn);
        save_setting!(automatically_fetch_metadata, self, txn);
        save_setting!(num_relays_per_person, self, txn);
        save_setting!(max_relays, self, txn);
        save_setting!(feed_chunk, self, txn);
        save_setting!(replies_chunk, self, txn);
        save_setting!(person_feed_chunk, self, txn);
        save_setting!(overlap, self, txn);
        save_setting!(custom_person_list_names, self, txn);
        save_setting!(reposts, self, txn);
        save_setting!(show_long_form, self, txn);
        save_setting!(show_mentions, self, txn);
        save_setting!(direct_messages, self, txn);
        save_setting!(future_allowance_secs, self, txn);
        save_setting!(hide_mutes_entirely, self, txn);
        save_setting!(reactions, self, txn);
        save_setting!(enable_zap_receipts, self, txn);
        save_setting!(show_media, self, txn);
        save_setting!(approve_content_warning, self, txn);
        save_setting!(show_deleted_events, self, txn);
        save_setting!(pow, self, txn);
        save_setting!(set_client_tag, self, txn);
        save_setting!(set_user_agent, self, txn);
        save_setting!(delegatee_tag, self, txn);
        save_setting!(max_fps, self, txn);
        save_setting!(recompute_feed_periodically, self, txn);
        save_setting!(feed_recompute_interval_ms, self, txn);
        save_setting!(theme_variant, self, txn);
        save_setting!(dark_mode, self, txn);
        save_setting!(follow_os_dark_mode, self, txn);
        save_setting!(override_dpi, self, txn);
        save_setting!(highlight_unread_events, self, txn);
        save_setting!(posting_area_at_top, self, txn);
        save_setting!(status_bar, self, txn);
        save_setting!(image_resize_algorithm, self, txn);
        save_setting!(inertial_scrolling, self, txn);
        save_setting!(mouse_acceleration, self, txn);
        save_setting!(relay_list_becomes_stale_hours, self, txn);
        save_setting!(metadata_becomes_stale_hours, self, txn);
        save_setting!(nip05_becomes_stale_if_valid_hours, self, txn);
        save_setting!(nip05_becomes_stale_if_invalid_minutes, self, txn);
        save_setting!(avatar_becomes_stale_hours, self, txn);
        save_setting!(media_becomes_stale_hours, self, txn);
        save_setting!(max_websocket_message_size_kb, self, txn);
        save_setting!(max_websocket_frame_size_kb, self, txn);
        save_setting!(websocket_accept_unmasked_frames, self, txn);
        save_setting!(websocket_connect_timeout_sec, self, txn);
        save_setting!(websocket_ping_frequency_sec, self, txn);
        save_setting!(fetcher_metadata_looptime_ms, self, txn);
        save_setting!(fetcher_looptime_ms, self, txn);
        save_setting!(fetcher_connect_timeout_sec, self, txn);
        save_setting!(fetcher_timeout_sec, self, txn);
        save_setting!(fetcher_max_requests_per_host, self, txn);
        save_setting!(fetcher_host_exclusion_on_low_error_secs, self, txn);
        save_setting!(fetcher_host_exclusion_on_med_error_secs, self, txn);
        save_setting!(fetcher_host_exclusion_on_high_error_secs, self, txn);
        save_setting!(nip11_lines_to_output_on_error, self, txn);
        save_setting!(prune_period_days, self, txn);
        save_setting!(cache_prune_period_days, self, txn);
        txn.commit()?;
        Ok(())
    }
}
