use super::Minion;
use crate::{BusMessage, Error};
use futures::SinkExt;
use nostr_types::{ClientMessage, Event, Id, IdHex, PublicKeyHex};
use tungstenite::protocol::Message as WsMessage;

impl Minion {
    pub(super) async fn handle_bus_message(
        &mut self,
        bus_message: BusMessage,
    ) -> Result<bool, Error> {
        match &*bus_message.kind {
            "shutdown" => {
                tracing::info!("{}: Websocket listener shutting down", &self.url);
                return Ok(false);
            }
            //"set_followed_people" => {
            //    let v: Vec<PublicKeyHex> = serde_json::from_str(&bus_message.json_payload)?;
            //    self.upsert_following(v).await?;
            //}
            "subscribe_general_feed" => {
                self.subscribe_general_feed().await?;
            }
            "subscribe_person_feed" => {
                let pubkeyhex: PublicKeyHex = serde_json::from_str(&bus_message.json_payload)?;
                self.subscribe_person_feed(pubkeyhex).await?;
            }
            "subscribe_thread_feed" => {
                let id: Id = serde_json::from_str(&bus_message.json_payload)?;
                self.subscribe_thread_feed(id).await?;
            }
            "fetch_events" => {
                let v: Vec<IdHex> = serde_json::from_str(&bus_message.json_payload)?;
                self.get_events(v).await?;
            }
            "post_event" => {
                let event: Event = serde_json::from_str(&bus_message.json_payload)?;
                let msg = ClientMessage::Event(Box::new(event));
                let wire = serde_json::to_string(&msg)?;
                let ws_sink = self.sink.as_mut().unwrap();
                ws_sink.send(WsMessage::Text(wire)).await?;
                tracing::info!("Posted event to {}", &self.url);
            }
            "temp_subscribe_metadata" => {
                let pubkeyhex: PublicKeyHex = serde_json::from_str(&bus_message.json_payload)?;
                self.temp_subscribe_metadata(pubkeyhex).await?;
            }
            _ => {
                tracing::warn!(
                    "{} Unrecognized bus message kind received by minion: {}",
                    &self.url,
                    bus_message.kind
                );
            }
        }
        Ok(true)
    }
}
