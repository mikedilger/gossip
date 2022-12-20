use super::Minion;
use crate::{BusMessage, Error};
use tracing::warn;

impl Minion {
    pub(super) async fn handle_bus_message(&self, bus_message: BusMessage) -> Result<(), Error> {
        warn!(
            "Websocket task got message, unimplemented: {}",
            bus_message.kind
        );
        Ok(())
    }
}
