use super::Minion;
use crate::{BusMessage, Error};
use nostr_types::{IdHex, PublicKeyHex};
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
            "fetch_events" => {
                let v: Vec<IdHex> = serde_json::from_str(&bus_message.json_payload)?;
                self.get_events(v).await?;
            }
            "follow_event_reactions" => {
                warn!("{}: follow event reactions unimplemented", &self.url);
            }
            _ => {
                warn!(
                    "{} Unrecognized bus message kind received by minion: {}",
                    &self.url, bus_message.kind
                );
            }
        }
        Ok(())
    }
}
