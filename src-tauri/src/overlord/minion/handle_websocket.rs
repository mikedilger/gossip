
use crate::{BusMessage, Error};
use crate::db::{DbEvent, DbPersonRelay};
use super::Minion;
use nostr_proto::{Event, RelayMessage, Unixtime};

impl Minion {
    pub(super) async fn handle_nostr_message(
        &mut self,
        ws_message: String
    ) -> Result<(), Error> {

        // TODO: pull out the raw event without any deserialization to be sure we don't mangle
        //       it.

        let relay_message: RelayMessage = serde_json::from_str(&ws_message)?;

        let mut maxtime = Unixtime::now()?;
        maxtime.0 += 60 * 15; // 15 minutes into the future

        match relay_message {
            RelayMessage::Event(_subid, event) => {
                if let Err(e) = event.verify(Some(maxtime)) {
                    log::error!("VERIFY ERROR: {}, {}", e, serde_json::to_string(&event)?)
                } else {
                    DbEvent::save_nostr_event(&*event, Some(self.url.clone())).await?;
                    self.send_overlord_newevent(*event).await?;
                }
            }
            RelayMessage::Notice(msg) => {
                log::info!("NOTICE: {} {}", &self.url, msg);
            }
            RelayMessage::Eose(subid) => {
                // We should update last_fetched
                let now = Unixtime::now().unwrap().0 as u64;
                DbPersonRelay::update_last_fetched(self.url.0.clone(), now).await?;

                // Update the matching subscription
                match self.subscriptions.get_mut(&subid.0) {
                    Some(sub) => {
                        sub.set_eose();
                        log::info!("EOSE: {} {:?}", &self.url, subid);
                    },
                    None => {
                        log::warn!("EOSE for unknown subsription: {} {:?}", &self.url, subid);
                    }
                }
            }
            RelayMessage::Ok(id, ok, ok_message) => {
                // These don't have to be processed.
                log::info!("OK: {} {:?} {} {}", &self.url, id, ok, ok_message);
            }
        }

        Ok(())
    }

    async fn send_overlord_newevent(
        &self,
        event: Event
    ) -> Result<(), Error> {
        self.to_overlord.send(BusMessage {
            relay_url: Some(self.url.0.clone()),
            target: "overlord".to_string(),
            kind: "new_event".to_string(),
            payload: serde_json::to_string(&event)?,
        })?;
        Ok(())
    }
}
