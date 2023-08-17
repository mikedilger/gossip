use crate::ui::{Theme, ThemeVariant};
use nostr_types::{EventKind, PublicKey};
use serde::{Deserialize, Serialize};
use speedy::{Readable, Writable};

#[derive(Clone, Debug, Serialize, Deserialize, Readable, Writable)]
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
            feed_recompute_interval_ms: 8000,
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
}
