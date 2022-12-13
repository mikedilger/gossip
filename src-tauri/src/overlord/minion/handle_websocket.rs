
use crate::{BusMessage, Error};
use crate::db::{DbEvent, DbPerson, DbPersonRelay};
use super::Minion;
use nostr_proto::{Event, EventKind, Metadata, RelayMessage, Unixtime};

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
                    if event.kind == EventKind::Metadata {
                        // We can handle these locally.
                        self.process_metadata_event(*event).await?;
                    } else {
                        self.send_overlord_newevent(*event).await?;
                    }
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

    async fn process_metadata_event(
        &self,
        event: Event
    ) -> Result<(), Error> {

        log::debug!("Event(metadata) from {}", &self.url);
        let metadata: Metadata = serde_json::from_str(&event.content)?;
        if let Some(mut person) = DbPerson::fetch_one(event.pubkey.into()).await? {
            if let Some(existing_at) = person.metadata_at {
                if event.created_at.0 <= existing_at {
                    // Old data. Ignore it
                    return Ok(());
                }
            }
            person.name = metadata.name;
            person.about = metadata.about;
            person.picture = metadata.picture;
            if person.dns_id != metadata.nip05 {
                person.dns_id = metadata.nip05;
                person.dns_id_valid = 0; // changed so starts invalid
                person.dns_id_last_checked = None; // we haven't checked this new one yet
            }
            person.metadata_at = Some(event.created_at.0);
            DbPerson::update(person.clone()).await?;
            self.send_javascript_setpeople(vec![person]).await?;
        } else {
            let mut person = DbPerson::new(event.pubkey.into());
            person.name = metadata.name;
            person.about = metadata.about;
            person.picture = metadata.picture;
            person.dns_id = metadata.nip05;
            person.metadata_at = Some(event.created_at.0);
            DbPerson::insert(person.clone()).await?;
            self.send_javascript_setpeople(vec![person]).await?;
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

    async fn send_javascript_setpeople(
        &self,
        people: Vec<DbPerson>
    ) -> Result<(), Error> {
        self.to_overlord.send(BusMessage {
            relay_url: Some(self.url.0.clone()),
            target: "javascript".to_string(),
            kind: "setpeople".to_string(),
            payload: serde_json::to_string(&people)?,
        })?;

        Ok(())
    }

}
