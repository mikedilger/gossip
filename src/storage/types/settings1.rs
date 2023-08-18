use crate::error::Error;
use super::super::Storage;
use super::theme1::{Theme1, ThemeVariant1};
use heed::RwTxn;
use nostr_types::PublicKey;
use serde::{Deserialize, Serialize};
use speedy::{Readable, Writable};

#[derive(Clone, Debug, Serialize, Deserialize, Readable, Writable)]
pub struct Settings1 {
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
    pub theme: Theme1,
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

impl Default for Settings1 {
    fn default() -> Settings1 {
        Settings1 {
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
            theme: Theme1 {
                variant: ThemeVariant1::Default,
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

impl Storage {
    #[allow(dead_code)]
    pub fn write_settings1<'a>(
        &'a self,
        settings: &Settings1,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let bytes = settings.write_to_vec()?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.general.put(txn, b"settings", &bytes)?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    #[allow(dead_code)]
    pub fn read_settings1(&self) -> Result<Option<Settings1>, Error> {
        let txn = self.env.read_txn()?;

        match self.general.get(&txn, b"settings")? {
            None => Ok(None),
            Some(bytes) => Ok(Some(Settings1::read_from_buffer(bytes)?)),
        }
    }

}
