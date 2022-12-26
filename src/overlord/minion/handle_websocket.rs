use super::Minion;
use crate::Error;
use nostr_types::{RelayMessage, Unixtime};
use tracing::{debug, error, info, warn};

impl Minion {
    pub(super) async fn handle_nostr_message(&mut self, ws_message: String) -> Result<(), Error> {
        // TODO: pull out the raw event without any deserialization to be sure we don't mangle
        //       it.

        let relay_message: RelayMessage = serde_json::from_str(&ws_message)?;

        let mut maxtime = Unixtime::now()?;
        maxtime.0 += 60 * 15; // 15 minutes into the future

        match relay_message {
            RelayMessage::Event(subid, event) => {
                if let Err(e) = event.verify(Some(maxtime)) {
                    error!("VERIFY ERROR: {}, {}", e, serde_json::to_string(&event)?)
                } else {
                    let handle = self
                        .subscriptions
                        .get_handle_by_id(&subid.0)
                        .unwrap_or_else(|| "_".to_owned());
                    debug!("NEW EVENT on {} [{}]", &self.url, handle);

                    crate::process::process_new_event(&event, true, Some(self.url.clone())).await?;
                }
            }
            RelayMessage::Notice(msg) => {
                info!("NOTICE: {} {}", &self.url, msg);
            }
            RelayMessage::Eose(subid) => {
                // Update the matching subscription
                match self.subscriptions.get_mut_by_id(&subid.0) {
                    Some(sub) => {
                        sub.set_eose();
                        info!("EOSE: {} {:?}", &self.url, subid);
                    }
                    None => {
                        warn!("EOSE for unknown subscription: {} {:?}", &self.url, subid);
                    }
                }
            }
            RelayMessage::Ok(id, ok, ok_message) => {
                // These don't have to be processed.
                info!("OK: {} {:?} {} {}", &self.url, id, ok, ok_message);
            }
        }

        Ok(())
    }
}
