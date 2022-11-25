use crate::Error;
use crate::db::DbSetting;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    pub feed_chunk: u64,
    pub overlap: u64,
    pub autofollow: u64,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            feed_chunk: crate::DEFAULT_FEED_CHUNK,
            overlap: crate::DEFAULT_OVERLAP,
            autofollow: 0
        }
    }
}

impl Settings {
    pub async fn load(&mut self) -> Result<(), Error> {
        self.feed_chunk = DbSetting::fetch_setting_u64_or_default(
            "feed_chunk",
            crate::DEFAULT_FEED_CHUNK
        ).await?;

        self.overlap = DbSetting::fetch_setting_u64_or_default(
            "overlap",
            crate::DEFAULT_OVERLAP
        ).await?;

        self.autofollow = DbSetting::fetch_setting_u64_or_default(
            "autofollow",
            crate::DEFAULT_AUTOFOLLOW
        ).await?;

        Ok(())
    }

    pub async fn save(&self) -> Result<(), Error> {
        DbSetting::update("feed_chunk".to_string(), self.feed_chunk).await?;
        DbSetting::update("overlap".to_string(), self.overlap).await?;
        DbSetting::update("autofollow".to_string(), self.autofollow).await?;
        Ok(())
    }
}
