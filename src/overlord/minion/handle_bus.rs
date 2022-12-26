use super::Minion;
use crate::{BusMessage, Error};
use nostr_types::PublicKeyHex;
use tracing::warn;

impl Minion {
    pub(super) async fn handle_bus_message(
        &mut self,
        bus_message: BusMessage,
    ) -> Result<(), Error> {
        match &*bus_message.kind {
            "set_followed_people" => {
                let v: Vec<PublicKeyHex> = serde_json::from_str(&bus_message.json_payload)?;
                self.upsert_following(v).await?;
            }
            "fetch_events" => {}
            "follow_event_reactions" => {}
            _ => {
                warn!(
                    "Unrecognized bus message kind received by minion: {}",
                    bus_message.kind
                );
            }
        }
        Ok(())
    }
}
