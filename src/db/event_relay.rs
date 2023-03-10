use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Id, RelayUrl};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DbEventRelay {
    pub event: String,
    pub relay: String,
    pub when_seen: u64,
}

impl DbEventRelay {
    pub async fn get_relays_for_event(id: Id) -> Result<Vec<RelayUrl>, Error> {
        let sql = "SELECT relay FROM event_relay WHERE event=?";

        let db = GLOBALS.db.get()?;
        let mut stmt = db.prepare(sql)?;
        stmt.raw_bind_parameter(1, id.as_hex_string())?;
        let mut rows = stmt.raw_query();
        let mut relays: Vec<RelayUrl> = Vec::new();
        while let Some(row) = rows.next()? {
            let s: String = row.get(0)?;
            // Just skip over bad relay URLs
            if let Ok(url) = RelayUrl::try_from_str(&s) {
                relays.push(url);
            }
        }
        Ok(relays)
    }

    pub async fn replace(event_relay: DbEventRelay) -> Result<(), Error> {
        let sql = "REPLACE INTO event_relay (event, relay, when_seen) \
             VALUES (?1, ?2, ?3)";

        let db = GLOBALS.db.get()?;

        let mut stmt = db.prepare(sql)?;
        stmt.execute((
            &event_relay.event,
            &event_relay.relay,
            &event_relay.when_seen,
        ))?;

        Ok(())
    }
}
