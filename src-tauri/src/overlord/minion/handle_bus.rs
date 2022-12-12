
use crate::{BusMessage, Error};
use super::Minion;

impl Minion {
    pub(super) async fn handle_bus_message(
        &self,
        bus_message: BusMessage
    ) -> Result<(), Error> {
        log::warn!("Websocket task got message, unimplemented: {}", bus_message.kind);
        Ok(())
    }
}
