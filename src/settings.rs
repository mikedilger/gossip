use crate::db::DbSetting;
use crate::error::Error;
use serde::{Deserialize, Serialize};

pub const DEFAULT_FEED_CHUNK: u64 = 43200; // 12 hours
pub const DEFAULT_OVERLAP: u64 = 600; // 10 minutes
pub const DEFAULT_AUTOFOLLOW: u64 = 0;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    pub feed_chunk: u64,
    pub overlap: u64,
    pub autofollow: u64,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            feed_chunk: DEFAULT_FEED_CHUNK,
            overlap: DEFAULT_OVERLAP,
            autofollow: 0,
        }
    }
}

impl Settings {
    #[allow(dead_code)]
    pub async fn load() -> Result<Settings, Error> {
        let feed_chunk =
            DbSetting::fetch_setting_u64_or_default("feed_chunk", DEFAULT_FEED_CHUNK).await?;

        let overlap = DbSetting::fetch_setting_u64_or_default("overlap", DEFAULT_OVERLAP).await?;

        let autofollow =
            DbSetting::fetch_setting_u64_or_default("autofollow", DEFAULT_AUTOFOLLOW).await?;

        Ok(Settings {
            feed_chunk,
            overlap,
            autofollow,
        })
    }

    #[allow(dead_code)]
    pub async fn save(&self) -> Result<(), Error> {
        DbSetting::update("feed_chunk".to_string(), self.feed_chunk).await?;
        DbSetting::update("overlap".to_string(), self.overlap).await?;
        DbSetting::update("autofollow".to_string(), self.autofollow).await?;
        Ok(())
    }
}
