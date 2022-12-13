
use crate::{BusMessage, Error, KeyPasswordPacket, PasswordPacket};
use crate::db::DbSetting;
use super::Overlord;
use nostr_proto::{Event, PrivateKey};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
struct PublicKeyInfo {
    public_key: String,
    key_security: u8
}

impl Overlord {
    pub(super) async fn handle_bus_message(&mut self, bus_message: BusMessage) -> Result<bool, Error> {
        match &*bus_message.target {
            "javascript" => {
                self.send_to_javascript(bus_message)?;
            }
            "all" => match &*bus_message.kind {
                "shutdown" => {
                    log::info!("Overlord shutting down");
                    return Ok(false);
                },
                "settings_changed" => {
                    self.settings = serde_json::from_str(&bus_message.payload)?;
                    // We need to inform the minions
                    self.to_minions.send(BusMessage {
                        relay_url: None,
                        target: "all".to_string(),
                        kind: "settings_changed".to_string(),
                        payload: bus_message.payload.clone(),
                    })?;
                },
                _ => {}
            },
            "overlord" => match &*bus_message.kind {
                "javascript_is_ready" => {
                    log::info!("Javascript is ready");
                    self.javascript_is_ready = true;
                    self.send_early_messages_to_javascript()?;
                },
                "minion_is_ready" => {
                    // We don't bother with this. We don't send minions messages
                    // early on. In the future when we spin up new minions
                    // after startup we may need this.
                },
                "new_event" => {
                    // FIXME - on startup, relays will stream a lot of events
                    //         quickly. Rather than making all these fast changes
                    //         on the frontend, maybe we should batch them up
                    //         and update the front end every 500ms or so?

                    let event: Event = serde_json::from_str(&bus_message.payload)?;
                    let changed_events = self.feed_event_processor.add_events(&[event]);

                    log::info!("Received new event from {} affecting {} events",
                               bus_message.relay_url.as_ref().unwrap(),
                               changed_events.len());

                    // Update javascript with added and changed events
                    self.send_to_javascript(BusMessage {
                        relay_url: None,
                        target: "javascript".to_string(),
                        kind: "setevents".to_string(),
                        payload: serde_json::to_string(&changed_events)?,
                    })?;

                    // Update javascript, replace feed
                    self.send_to_javascript(BusMessage {
                        relay_url: None,
                        target: "javascript".to_string(),
                        kind: "replacefeed".to_string(),
                        payload: serde_json::to_string(&self.feed_event_processor.get_feed())?,
                    })?;
                },
                "generate" => {
                    let password: PasswordPacket = serde_json::from_str(&bus_message.payload)?;

                    self.private_key = Some(PrivateKey::generate());

                    let encrypted_private_key = self.private_key.as_ref().unwrap()
                        .export_encrypted(&password.0)?;

                    let user_private_key_setting = DbSetting {
                        key: "user_private_key".to_string(),
                        value: encrypted_private_key.clone()
                    };

                    DbSetting::set(user_private_key_setting).await?;

                    let pkref = self.private_key.as_ref().unwrap();
                    let pki = PublicKeyInfo {
                        public_key: pkref.public_key().as_hex_string(),
                        key_security: pkref.key_security() as u8
                    };

                    // Let javascript know our public key
                    self.send_to_javascript(BusMessage {
                        relay_url: None,
                        target: "javascript".to_string(),
                        kind: "publickey".to_string(),
                        payload: serde_json::to_string(&pki)?
                    })?;
                },
                "unlock" => {
                    let password: PasswordPacket = serde_json::from_str(&bus_message.payload)?;

                    if let Some(epk) = DbSetting::fetch_setting("user_private_key").await? {
                        self.private_key = Some(PrivateKey::import_encrypted(&epk, &password.0)?);

                        let pkref = self.private_key.as_ref().unwrap();
                        let pki = PublicKeyInfo {
                            public_key: pkref.public_key().as_hex_string(),
                            key_security: pkref.key_security() as u8
                        };

                        // Let javascript know our public key
                        self.send_to_javascript(BusMessage {
                            relay_url: None,
                            target: "javascript".to_string(),
                            kind: "publickey".to_string(),
                            payload: serde_json::to_string(&pki)?
                        })?;
                    }
                },
                "import_key" => {
                    let key_password_packet: KeyPasswordPacket =
                        serde_json::from_str(&bus_message.payload)?;

                    self.private_key = Some(
                        PrivateKey::try_from_hex_string(&key_password_packet.0)?
                    );

                    let encrypted_private_key = self.private_key.as_ref().unwrap()
                        .export_encrypted(&key_password_packet.1)?;

                    let user_private_key_setting = DbSetting {
                        key: "user_private_key".to_string(),
                        value: encrypted_private_key.clone()
                    };

                    DbSetting::set(user_private_key_setting).await?;

                    let pkref = self.private_key.as_ref().unwrap();
                    let pki = PublicKeyInfo {
                        public_key: pkref.public_key().as_hex_string(),
                        key_security: pkref.key_security() as u8
                    };

                    // Let javascript know our public key
                    self.send_to_javascript(BusMessage {
                        relay_url: None,
                        target: "javascript".to_string(),
                        kind: "publickey".to_string(),
                        payload: serde_json::to_string(&pki)?
                    })?;
                }
                _ => {}
            },
            _ => {}
        }

        Ok(true)
    }

}
