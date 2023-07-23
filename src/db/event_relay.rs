use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Id, RelayUrl, Unixtime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DbEventRelay {
    pub id: Id,
    pub relay: RelayUrl,
    pub when_seen: Unixtime,
}

impl DbEventRelay {
    pub fn get_relays_for_event(id: Id) -> Result<Vec<RelayUrl>, Error> {
        Ok(GLOBALS
            .storage
            .get_event_seen_on_relay(id)?
            .drain(..)
            .map(|(url, _time)| url)
            .collect())
    }
}
