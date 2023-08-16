use crate::ui::{Theme, ThemeVariant};
use nostr_types::{EventKind, PublicKey};
use serde::{Deserialize, Serialize};
use speedy::{Readable, Writable};

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

    // Event Selection
    pub reposts: bool,
    pub show_long_form: bool,
    pub show_mentions: bool,
    pub direct_messages: bool,
    pub future_allowance_secs: u64,

    // Event Content Settings
    pub reactions: bool,
    pub enable_zap_receipts: bool,
    pub show_media: bool,

    // Posting Settings
    pub pow: u8,
    pub set_client_tag: bool,
    pub set_user_agent: bool,
    pub delegatee_tag: String,

    // UI settings
    pub max_fps: u32,
    pub recompute_feed_periodically: bool,
    pub feed_recompute_interval_ms: u32,
    pub theme: Theme,
    pub override_dpi: Option<u32>,
    pub highlight_unread_events: bool,
    pub posting_area_at_top: bool,
    pub status_bar: bool,
    pub image_resize_algorithm: String,

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
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            // ID settings
            public_key: None,
            log_n: 18,

            // Network settings
            offline: false,
            load_avatars: true,
            load_media: true,
            check_nip05: true,
            automatically_fetch_metadata: true,

            // Relay settings
            num_relays_per_person: 2,
            max_relays: 50,

            // Feed settings
            feed_chunk: 60 * 60 * 12,             // 12 hours
            replies_chunk: 60 * 60 * 24 * 7,      // 1 week
            person_feed_chunk: 60 * 60 * 24 * 30, // 1 month
            overlap: 300,                         // 5 minutes

            // Event Selection
            reposts: true,
            show_long_form: false,
            show_mentions: true,
            direct_messages: true,
            future_allowance_secs: 60 * 15, // 15 minutes

            // Event Content Settings
            reactions: true,
            enable_zap_receipts: true,
            show_media: true,

            // Posting settings
            pow: 0,
            set_client_tag: false,
            set_user_agent: false,
            delegatee_tag: String::new(),

            // UI settings
            max_fps: 12,
            recompute_feed_periodically: true,
            feed_recompute_interval_ms: 8000,
            theme: Theme {
                variant: ThemeVariant::Default,
                dark_mode: false,
                follow_os_dark_mode: false,
            },
            override_dpi: None,
            highlight_unread_events: true,
            posting_area_at_top: true,
            status_bar: false,
            image_resize_algorithm: "CatmullRom".to_owned(),

            // Staletime settings
            relay_list_becomes_stale_hours: 8,
            metadata_becomes_stale_hours: 8,
            nip05_becomes_stale_if_valid_hours: 8,
            nip05_becomes_stale_if_invalid_minutes: 30, // 30 minutes
            avatar_becomes_stale_hours: 8,
            media_becomes_stale_hours: 8,

            // Websocket settings
            max_websocket_message_size_kb: 1024, // 1 MB
            max_websocket_frame_size_kb: 1024,   // 1 MB
            websocket_accept_unmasked_frames: false,
            websocket_connect_timeout_sec: 15,
            websocket_ping_frequency_sec: 55,

            // HTTP settings
            fetcher_metadata_looptime_ms: 3000,
            fetcher_looptime_ms: 1800,
            fetcher_connect_timeout_sec: 15,
            fetcher_timeout_sec: 30,
            fetcher_max_requests_per_host: 3,
            fetcher_host_exclusion_on_low_error_secs: 30,
            fetcher_host_exclusion_on_med_error_secs: 60,
            fetcher_host_exclusion_on_high_error_secs: 600,
            nip11_lines_to_output_on_error: 10,

            // Database settings
            prune_period_days: 30,
        }
    }
}

impl Settings {
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

    pub fn feed_displayable_event_kinds(&self) -> Vec<EventKind> {
        self.enabled_event_kinds()
            .drain(..)
            .filter(|k| k.is_feed_displayable())
            .collect()
    }

    pub fn feed_augment_event_kinds(&self) -> Vec<EventKind> {
        self.enabled_event_kinds()
            .drain(..)
            .filter(|k| k.augments_feed_related())
            .collect()
    }
}
