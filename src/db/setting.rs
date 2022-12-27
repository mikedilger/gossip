use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DbSetting {
    pub key: String,
    pub value: String,
}
