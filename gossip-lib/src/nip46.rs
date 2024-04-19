use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use crate::{Error, ErrorKind};
use nostr_types::{
    ContentEncryptionAlgorithm, Event, EventKind, PreEvent, PublicKey, RelayUrl, Tag, Unixtime,
};
use serde::Deserialize;
use speedy::{Readable, Writable};

/// This is a server not yet connected, ready to be connected
#[derive(Debug, Clone, Readable, Writable)]
pub struct Nip46UnconnectedServer {
    pub connect_secret: String,
    pub name: String,
    pub relays: Vec<RelayUrl>,
}

impl Nip46UnconnectedServer {
    pub fn new(name: String, relays: Vec<RelayUrl>) -> Nip46UnconnectedServer {
        let connect_secret = textnonce::TextNonce::sized_urlsafe(32)
            .unwrap()
            .into_string();

        Nip46UnconnectedServer {
            connect_secret,
            name,
            relays,
        }
    }

    pub fn connection_token(&self) -> Result<String, Error> {
        let public_key = match GLOBALS.storage.read_setting_public_key() {
            Some(pk) => pk,
            None => return Err(ErrorKind::NoPublicKey.into()),
        };

        let relay_part = &self
            .relays
            .iter()
            .map(|r| format!("relay={}", r))
            .collect::<Vec<String>>()
            .join("&");

        let token = format!(
            "bunker://{}?{}&secret={}",
            public_key.as_hex_string(),
            relay_part,
            self.connect_secret
        );

        Ok(token)
    }
}

#[derive(Debug, Default, Copy, Clone, Readable, Writable, PartialEq, Eq)]
pub enum Approval {
    None,
    Once,
    Until(Unixtime),
    Always,
    #[default]
    Ask,
}

impl Approval {
    fn is_approved(&mut self) -> bool {
        match self {
            Approval::None => false,
            Approval::Once => {
                *self = Approval::None;
                true
            }
            Approval::Until(time) => {
                let approved = Unixtime::now().unwrap() < *time;
                if !approved {
                    *self = Approval::None;
                }
                approved
            }
            Approval::Always => true,
            Approval::Ask => false,
        }
    }
}

#[derive(Debug, Clone, Readable, Writable)]
pub struct Nip46Server {
    pub name: String,
    pub peer_pubkey: PublicKey,
    pub relays: Vec<RelayUrl>,
    pub sign_approval: Approval,
    pub encrypt_approval: Approval,
    pub decrypt_approval: Approval,
}

impl Nip46Server {
    pub fn handle(&mut self, cmd: &ParsedCommand) -> Result<(), Error> {
        let ParsedCommand {
            ref id,
            ref method,
            ref params,
        } = cmd;

        let result: Result<String, Error> = match method.as_str() {
            "connect" => Err("You are already connected".into()),
            "get_public_key" => self.get_public_key(),
            "sign_event" => {
                if self.sign_approval.is_approved() {
                    self.sign_event(params)
                } else if self.sign_approval == Approval::Ask {
                    Err(ErrorKind::Nip46NeedApproval.into())
                } else {
                    Err(ErrorKind::Nip46Denied.into())
                }
            }
            "get_relays" => self.get_relays(),
            "nip04_encrypt" => {
                if self.encrypt_approval.is_approved() {
                    self.nip04_encrypt(params)
                } else if self.encrypt_approval == Approval::Ask {
                    Err(ErrorKind::Nip46NeedApproval.into())
                } else {
                    Err(ErrorKind::Nip46Denied.into())
                }
            }
            "nip04_decrypt" => {
                if self.decrypt_approval.is_approved() {
                    self.nip04_decrypt(params)
                } else if self.decrypt_approval == Approval::Ask {
                    Err(ErrorKind::Nip46NeedApproval.into())
                } else {
                    Err(ErrorKind::Nip46Denied.into())
                }
            }
            "nip44_get_key" => {
                if self.encrypt_approval.is_approved() || self.decrypt_approval.is_approved() {
                    self.nip44_get_key(params)
                } else if self.encrypt_approval == Approval::Ask {
                    Err(ErrorKind::Nip46NeedApproval.into())
                } else {
                    Err(ErrorKind::Nip46Denied.into())
                }
            }
            "nip44_encrypt" => {
                if self.encrypt_approval.is_approved() {
                    self.nip44_encrypt(params)
                } else if self.encrypt_approval == Approval::Ask {
                    Err(ErrorKind::Nip46NeedApproval.into())
                } else {
                    Err(ErrorKind::Nip46Denied.into())
                }
            }
            "nip44_decrypt" => {
                if self.decrypt_approval.is_approved() {
                    self.nip44_decrypt(params)
                } else if self.decrypt_approval == Approval::Ask {
                    Err(ErrorKind::Nip46NeedApproval.into())
                } else {
                    Err(ErrorKind::Nip46Denied.into())
                }
            }
            "ping" => self.ping(),
            _ => Err("unrecognized command".into()),
        };

        match result {
            Ok(answer) => send_response(
                id.to_owned(),
                answer,
                "".to_owned(),
                self.peer_pubkey,
                self.relays.clone(),
            )?,
            Err(e) => {
                if matches!(e.kind, ErrorKind::Nip46NeedApproval) {
                    return Err(e);
                } else {
                    send_response(
                        id.to_owned(),
                        "".to_owned(),
                        format!("{}", e),
                        self.peer_pubkey,
                        self.relays.clone(),
                    )?;
                }
            }
        }

        Ok(())
    }

    fn get_public_key(&self) -> Result<String, Error> {
        if let Some(pk) = GLOBALS.identity.public_key() {
            Ok(pk.as_hex_string())
        } else {
            Err("No public key configured".into())
        }
    }

    fn sign_event(&self, params: &[String]) -> Result<String, Error> {
        if params.is_empty() {
            return Err("sign_event: requires a parameter".into());
        }

        let public_key = match GLOBALS.storage.read_setting_public_key() {
            Some(pk) => pk,
            None => return Err(ErrorKind::NoPublicKey.into()),
        };

        let Nip46PreEvent {
            pubkey,
            created_at,
            kind,
            tags,
            content,
        } = serde_json::from_str(&params[0])?;

        if let Some(pk) = pubkey {
            if pk != public_key {
                return Err("sign_event: pubkey mismatch".into());
            }
        }

        let pre_event: PreEvent = PreEvent {
            pubkey: public_key,
            created_at: created_at.unwrap_or(Unixtime::now().unwrap()),
            kind,
            tags,
            content,
        };

        let event = GLOBALS.identity.sign_event(pre_event)?;

        let event_str = serde_json::to_string(&event)?;

        Ok(event_str)
    }

    fn get_relays(&self) -> Result<String, Error> {
        let answer = serde_json::to_string(&self.relays)?;
        Ok(answer)
    }

    fn nip04_encrypt(&self, params: &[String]) -> Result<String, Error> {
        if params.len() < 2 {
            return Err("nip04_encrypt: requires two parameters".into());
        }
        let other_pubkey = PublicKey::try_from_hex_string(&params[0], true)?;
        let ciphertext = GLOBALS.identity.encrypt(
            &other_pubkey,
            &params[1],
            ContentEncryptionAlgorithm::Nip04,
        )?;
        Ok(ciphertext)
    }

    fn nip04_decrypt(&self, params: &[String]) -> Result<String, Error> {
        if params.len() < 2 {
            return Err("nip04_decrypt: requires two parameters".into());
        }
        let other_pubkey = PublicKey::try_from_hex_string(&params[0], true)?;
        GLOBALS.identity.decrypt(&other_pubkey, &params[1])
    }

    fn nip44_get_key(&self, params: &[String]) -> Result<String, Error> {
        if params.is_empty() {
            return Err("nip44_get_key: requires a parameter".into());
        }
        let other_pubkey = PublicKey::try_from_hex_string(&params[0], true)?;
        let ck = GLOBALS.identity.nip44_conversation_key(&other_pubkey)?;
        let ckhex = hex::encode(ck);
        Ok(ckhex)
    }

    fn nip44_encrypt(&self, params: &[String]) -> Result<String, Error> {
        if params.len() < 2 {
            return Err("nip44_encrypt: requires two parameters".into());
        }
        let other_pubkey = PublicKey::try_from_hex_string(&params[0], true)?;
        let ciphertext = GLOBALS.identity.encrypt(
            &other_pubkey,
            &params[1],
            ContentEncryptionAlgorithm::Nip44v2,
        )?;
        Ok(ciphertext)
    }

    fn nip44_decrypt(&self, params: &[String]) -> Result<String, Error> {
        if params.len() < 2 {
            return Err("nip44_decrypt: requires two parameters".into());
        }
        let other_pubkey = PublicKey::try_from_hex_string(&params[0], true)?;
        let plaintext = GLOBALS.identity.decrypt(&other_pubkey, &params[1])?;
        Ok(plaintext)
    }

    fn ping(&self) -> Result<String, Error> {
        Ok("pong".to_owned())
    }
}

#[derive(Debug, Deserialize)]
pub struct Nip46PreEvent {
    #[serde(default)]
    pub pubkey: Option<PublicKey>,

    #[serde(default = "default_now")]
    pub created_at: Option<Unixtime>,

    pub kind: EventKind,

    pub tags: Vec<Tag>,

    pub content: String,
}

fn default_now() -> Option<Unixtime> {
    Some(Unixtime::now().unwrap())
}

#[derive(Debug, Clone, Hash, PartialEq)]
pub struct ParsedCommand {
    pub id: String,
    pub method: String,
    pub params: Vec<String>,
}

fn parse_command(peer_pubkey: PublicKey, contents: &str) -> Result<ParsedCommand, Error> {
    let bytes = GLOBALS.identity.decrypt(&peer_pubkey, contents)?;

    let json: serde_json::Value = serde_json::from_str(&bytes)?;

    let map = match json.as_object() {
        Some(map) => map,
        None => return Err(ErrorKind::Nip46CommandNotJsonObject.into()),
    };

    let id: String = match map.get("id") {
        Some(id) => match id.as_str() {
            Some(s) => s.to_owned(),
            None => return Err(ErrorKind::Nip46CommandMissingId.into()),
        },
        None => return Err(ErrorKind::Nip46CommandMissingId.into()),
    };

    let method: String = match map.get("method") {
        Some(method) => match method.as_str() {
            Some(s) => s.to_owned(),
            None => {
                return Err(
                    ErrorKind::Nip46ParsingError(id, "method not a string".to_owned()).into(),
                )
            }
        },
        None => {
            return Err(
                ErrorKind::Nip46ParsingError(id, "method parameter missing".to_owned()).into(),
            )
        }
    };

    let mut params: Vec<String> = Vec::new();
    match map.get("params") {
        Some(ps) => match ps.as_array() {
            Some(arr) => {
                for elem in arr {
                    match elem.as_str() {
                        Some(s) => params.push(s.to_owned()),
                        None => {
                            return Err(ErrorKind::Nip46ParsingError(
                                id,
                                "non-string parameter found".to_owned(),
                            )
                            .into())
                        }
                    }
                }
                Ok(ParsedCommand { id, method, params })
            }
            None => Err(ErrorKind::Nip46ParsingError(id, "params not an array".to_owned()).into()),
        },
        None => Err(ErrorKind::Nip46ParsingError(id, "params missing".to_owned()).into()),
    }
}

fn send_response(
    id: String,
    result: String,
    error: String,
    peer_pubkey: PublicKey,
    relays: Vec<RelayUrl>,
) -> Result<(), Error> {
    use serde_json::json;

    let public_key = match GLOBALS.storage.read_setting_public_key() {
        Some(pk) => pk,
        None => return Err(ErrorKind::NoPublicKey.into()),
    };

    let output = json!({
        "id": id,
        "result": result,
        "error": error
    });
    let s = output.to_string();

    let e = GLOBALS
        .identity
        .encrypt(&peer_pubkey, &s, ContentEncryptionAlgorithm::Nip04)?;

    let pre_event = PreEvent {
        pubkey: public_key,
        created_at: Unixtime::now().unwrap(),
        kind: EventKind::NostrConnect,
        tags: vec![Tag::new_pubkey(peer_pubkey, None, None)],
        content: e,
    };

    let event = GLOBALS.identity.sign_event(pre_event)?;

    GLOBALS
        .to_overlord
        .send(ToOverlordMessage::PostNip46Event(event, relays))?;

    Ok(())
}

pub fn handle_command(event: &Event, seen_on: Option<RelayUrl>) -> Result<(), Error> {
    // If we have a server for that pubkey
    if let Some(mut server) = GLOBALS.storage.read_nip46server(event.pubkey)? {
        // Parse the command
        let parsed_command = match parse_command(event.pubkey, &event.content) {
            Ok(pc) => pc,
            Err(e) => {
                if let ErrorKind::Nip46ParsingError(ref id, ref msg) = e.kind {
                    // Send back the error
                    send_response(
                        id.to_string(),
                        "".to_owned(),
                        msg.clone(),
                        event.pubkey,
                        server.relays.clone(),
                    )?;
                }

                // Return the error
                return Err(e);
            }
        };

        // Handle the command
        if let Err(e) = server.handle(&parsed_command) {
            if matches!(e.kind, ErrorKind::Nip46NeedApproval) {
                GLOBALS
                    .pending
                    .insert(crate::pending::PendingItem::Nip46Request {
                        client_name: server.name.clone(),
                        account: event.pubkey,
                        command: parsed_command,
                    });
            } else {
                // Return the error
                return Err(e);
            }
        }

        return Ok(());
    }

    // Make sure we have a relay to reply on for early errors
    let seen_on_relay = match seen_on {
        Some(r) => r,
        None => return Err(ErrorKind::Nip46RelayNeeded.into()),
    };

    // Check for a `connect` command
    // which is the only command available to unconfigured pubkeys
    let parsed_command = match parse_command(event.pubkey, &event.content) {
        Ok(parsed_command) => parsed_command,
        Err(e) => {
            // Send back the error if we have one for them
            if let ErrorKind::Nip46ParsingError(ref id, ref msg) = e.kind {
                send_response(
                    id.to_string(),
                    "".to_owned(),
                    msg.clone(),
                    event.pubkey,
                    vec![seen_on_relay],
                )?;
            }

            // And return the error
            return Err(e);
        }
    };

    let ParsedCommand { id, method, params } = parsed_command;

    // Do we have a waiiting unconnected server?
    let userver = match GLOBALS.storage.read_nip46_unconnected_server()? {
        Some(userver) => userver,
        None => {
            // We aren't setup to receive a connection
            send_response(
                id.clone(),
                "".to_owned(),
                "Gossip is not configured to receive a connection".to_string(),
                event.pubkey,
                vec![seen_on_relay],
            )?;
            return Ok(()); // no need to pass back error
        }
    };

    // Combine userver.relays and seen_on_relay
    let mut reply_relays = userver.relays.clone();
    reply_relays.push(seen_on_relay);
    reply_relays.sort();
    reply_relays.dedup();

    if method != "connect" {
        send_response(
            id.clone(),
            "".to_owned(),
            "Your pubkey is not configured for nostr connect here.".to_string(),
            event.pubkey,
            reply_relays,
        )?;
        return Ok(()); // no need to pass back error
    }

    if params.len() < 2 {
        send_response(
            id.clone(),
            "".to_owned(),
            "connect requires two parameters".to_string(),
            event.pubkey,
            reply_relays,
        )?;
        return Ok(()); // no need to pass back error
    }

    let public_key = match GLOBALS.storage.read_setting_public_key() {
        Some(pk) => pk,
        None => {
            send_response(
                id.clone(),
                "".to_owned(),
                "No public key".to_string(),
                event.pubkey,
                reply_relays,
            )?;
            return Err(ErrorKind::NoPublicKey.into());
        }
    };

    if params[0] != public_key.as_hex_string() {
        // We aren't setup to receive a connection
        send_response(
            id.clone(),
            "".to_owned(),
            "Gossip is not configured to sign with the requested public key".to_string(),
            event.pubkey,
            reply_relays,
        )?;
        return Ok(()); // no need to pass back error
    }

    if params[1] != userver.connect_secret.as_str() {
        send_response(
            id.clone(),
            "".to_owned(),
            "Incorrect secret.".to_string(),
            event.pubkey,
            reply_relays,
        )?;
        return Ok(()); // no need to pass back error
    }

    // Turn it into a full server
    let server = Nip46Server {
        name: userver.name,
        peer_pubkey: event.pubkey,
        relays: reply_relays.clone(),
        sign_approval: Approval::Ask,
        encrypt_approval: Approval::Ask,
        decrypt_approval: Approval::Ask,
    };

    // Save the server, and delete the unconnected server
    let mut txn = GLOBALS.storage.get_write_txn()?;
    GLOBALS.storage.write_nip46server(&server, Some(&mut txn))?;
    GLOBALS
        .storage
        .delete_nip46_unconnected_server(Some(&mut txn))?;
    txn.commit()?;

    // Acknowledge
    send_response(
        id.clone(),
        "ack".to_owned(),
        "".to_owned(),
        event.pubkey,
        reply_relays,
    )?;

    Ok(())
}
