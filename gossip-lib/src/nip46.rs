use crate::globals::GLOBALS;
use crate::{Error, ErrorKind};
use nostr_types::{
    ContentEncryptionAlgorithm, EventKind, PreEvent, PublicKey, RelayUrl, Tag, Unixtime,
};
use serde::{Deserialize, Serialize};
use speedy::{Readable, Writable};

#[derive(Debug, Clone, Readable, Writable, Serialize, Deserialize)]
pub struct Nip46ClientMetadata {
    pub name: String,
    pub url: RelayUrl,
    pub description: String,
}

/// This is a server not yet connected, ready to be connected
#[derive(Debug, Clone, Readable, Writable)]
pub struct Nip46UnconnectedServer {
    pub connect_secret: String,
    pub relays: Vec<RelayUrl>,
}

impl Nip46UnconnectedServer {
    pub fn new(relays: Vec<RelayUrl>) -> Nip46UnconnectedServer {
        let connect_secret = textnonce::TextNonce::sized_urlsafe(32)
            .unwrap()
            .into_string();

        Nip46UnconnectedServer {
            connect_secret,
            relays,
        }
    }

    pub fn connection_token(&self) -> Result<String, Error> {
        let public_key = match GLOBALS.storage.read_setting_public_key() {
            Some(pk) => pk,
            None => return Err(ErrorKind::NoPublicKey.into()),
        };

        let mut token = format!("{}#{}?", public_key.as_bech32_string(), self.connect_secret);

        token.push_str(
            &self
                .relays
                .iter()
                .map(|r| format!("relay={}", r))
                .collect::<Vec<String>>()
                .join("&"),
        );

        Ok(token)
    }
}

#[derive(Debug, Clone, Readable, Writable)]
pub struct Nip46Server {
    pub peer_pubkey: PublicKey,
    pub relays: Vec<RelayUrl>,
    pub metadata: Option<Nip46ClientMetadata>,
}

impl Nip46Server {
    pub fn new_from_client(input: String) -> Result<Nip46Server, Error> {
        // nostrconnect://<client-key-hex>?relay=wss://...&metadata={"name":"...", "url": "...", "description": "..."}

        // "nostrconnect://"
        if !input.starts_with("nostrconnect://") {
            return Err(ErrorKind::BadNostrConnectString.into());
        }
        let mut pos = 15;

        // client-key-kex
        if input.len() < pos + 64 {
            return Err(ErrorKind::BadNostrConnectString.into());
        }
        let peer_pubkey = PublicKey::try_from_hex_string(&input[pos..pos + 64], true)?;
        pos += 64;

        // '?'
        if input.len() < pos + 1 {
            return Err(ErrorKind::BadNostrConnectString.into());
        }
        if &input[pos..pos + 1] != "?" {
            return Err(ErrorKind::BadNostrConnectString.into());
        }
        pos += 1;

        let mut relays: Vec<RelayUrl> = Vec::new();
        let mut metadata: Option<Nip46ClientMetadata> = None;

        loop {
            if &input[pos..pos + 6] == "relay=" {
                pos += 6;
                if let Some(amp) = input[pos..].find('&') {
                    relays.push(RelayUrl::try_from_str(&input[pos..amp])?);
                    pos += amp;
                } else {
                    relays.push(RelayUrl::try_from_str(&input[pos..])?);
                    break;
                }
            } else if &input[pos..pos + 9] == "metadata=" {
                pos += 9;
                metadata = Some(serde_json::from_str(&input[pos..])?);
                break;
            } else {
                // FIXME, we should tolerate unknown fields
                return Err(ErrorKind::BadNostrConnectString.into());
            }
        }

        Ok(Nip46Server {
            peer_pubkey,
            relays,
            metadata,
        })
    }

    fn get_public_key(&self, _params: Vec<String>) -> Result<String, Error> {
        if let Some(pk) = GLOBALS.identity.public_key() {
            Ok(pk.as_hex_string())
        } else {
            Err("No public key configured".into())
        }
    }

    fn sign_event(&self, params: Vec<String>) -> Result<String, Error> {
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

    fn get_relays(&self, _params: Vec<String>) -> Result<String, Error> {
        let answer = serde_json::to_string(&self.relays)?;
        Ok(answer)
    }

    fn nip04_encrypt(&self, params: Vec<String>) -> Result<String, Error> {
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

    fn nip04_decrypt(&self, params: Vec<String>) -> Result<String, Error> {
        if params.len() < 2 {
            return Err("nip04_decrypt: requires two parameters".into());
        }
        let other_pubkey = PublicKey::try_from_hex_string(&params[0], true)?;
        let plaintext_bytes = GLOBALS.identity.decrypt_nip04(&other_pubkey, &params[1])?;
        let utf8 = String::from_utf8(plaintext_bytes)?;
        Ok(utf8)
    }

    fn nip44_get_key(&self, params: Vec<String>) -> Result<String, Error> {
        if params.is_empty() {
            return Err("nip44_get_key: requires a parameter".into());
        }
        let other_pubkey = PublicKey::try_from_hex_string(&params[0], true)?;
        let ck = GLOBALS.identity.nip44_conversation_key(&other_pubkey)?;
        let ckhex = hex::encode(ck);
        Ok(ckhex)
    }

    fn nip44_encrypt(&self, params: Vec<String>) -> Result<String, Error> {
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

    fn nip44_decrypt(&self, params: Vec<String>) -> Result<String, Error> {
        if params.len() < 2 {
            return Err("nip44_decrypt: requires two parameters".into());
        }
        let other_pubkey = PublicKey::try_from_hex_string(&params[0], true)?;
        let plaintext = GLOBALS.identity.decrypt_nip44(&other_pubkey, &params[1])?;
        Ok(plaintext)
    }

    fn ping(&self, _params: Vec<String>) -> Result<String, Error> {
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
