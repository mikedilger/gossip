
use crate::{BusMessage, Error};
use crate::db::{DbEvent, DbEventSeen, DbEventTag, DbPerson, DbPersonRelay};
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
                    self.save_event_in_database(&*event).await?;
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

    async fn save_event_in_database(
        &self,
        event: &Event
    ) -> Result<(), Error> {
        let db_event = DbEvent {
            id: event.id.into(),
            raw: serde_json::to_string(&event)?, // TODO: this is reserialized.
            pubkey: event.pubkey.into(),
            created_at: event.created_at.0,
            kind: event.kind.into(),
            content: event.content.clone(),
            ots: event.ots.clone()
        };
        DbEvent::insert(db_event).await?;

        let mut seq = 0;
        for tag in event.tags.iter() {
            // convert to vec of strings
            let v: Vec<String> = serde_json::from_str(&serde_json::to_string(&tag)?)?;

            let db_event_tag = DbEventTag {
                event: event.id.as_hex_string(),
                seq: seq,
                label: v.get(0).cloned(),
                field0: v.get(1).cloned(),
                field1: v.get(2).cloned(),
                field2: v.get(3).cloned(),
                field3: v.get(4).cloned(),
            };
            DbEventTag::insert(db_event_tag).await?;
            seq += 1;
        }

        let db_event_seen = DbEventSeen {
            event: event.id.as_hex_string(),
            relay: self.url.0.clone(),
            when_seen: Unixtime::now()?.0 as u64
        };
        DbEventSeen::replace(db_event_seen).await?;

        Ok(())
    }

    async fn process_metadata_event(
        &self,
        event: Event
    ) -> Result<(), Error> {

        log::debug!("Event(metadata) from {}", &self.url);
        let created_at: u64 = event.created_at.0 as u64;
        let metadata: Metadata = serde_json::from_str(&event.content)?;
        if let Some(mut person) = DbPerson::fetch_one(event.pubkey.into()).await? {
            person.name = Some(metadata.name);
            person.about = metadata.about;
            person.picture = metadata.picture;
            if person.dns_id != metadata.nip05 {
                person.dns_id = metadata.nip05;
                person.dns_id_valid = 0; // changed so starts invalid
                person.dns_id_last_checked = match person.dns_id_last_checked {
                    None => Some(created_at),
                    Some(lc) => Some(created_at.max(lc)),
                }
            }
            DbPerson::update(person.clone()).await?;
            self.send_javascript_setpeople(vec![person]).await?;
        } else {
            let person = DbPerson {
                pubkey: event.pubkey.into(),
                name: Some(metadata.name),
                about: metadata.about,
                picture: metadata.picture,
                dns_id: metadata.nip05,
                dns_id_valid: 0, // new so starts invalid
                dns_id_last_checked: Some(created_at),
                followed: 0
            };
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
