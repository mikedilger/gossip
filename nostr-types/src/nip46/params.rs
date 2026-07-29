use crate::{Error, PublicKey, RelayUrl};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

/// The connection parameters provided by an nsec bunker for a client connecting to it
/// usually as a `bunker://` url
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Nip46ConnectionParameters {
    /// The public key of the remote signer
    pub remote_signer_pubkey: PublicKey,

    /// The relays to contact the remote signer on
    pub relays: Vec<RelayUrl>,

    /// A secret to provide in the connect request to prove this client is authorized
    pub secret: Option<String>,
}

impl Nip46ConnectionParameters {
    /// Parse a `bunker://` url into `Nip46ConnectionParameters`
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Nip46ConnectionParameters, Error> {
        // "bunker://{pk}?{relay_part}&secret={secret}"
        use regex::Regex;
        lazy_static! {
            static ref BUNKER_RE: Regex =
                Regex::new(r#"^bunker://(.+)\?(.+)$"#).expect("Could not compile bunker regex");
        }

        let mut relays: Vec<RelayUrl> = Vec::new();
        let mut secret: Option<String> = None;

        let captures = match BUNKER_RE.captures(s) {
            Some(c) => c,
            None => return Err(Error::BadBunkerUrl),
        };

        let public_key = if let Some(pk_part) = captures.get(1) {
            PublicKey::try_from_hex_string(pk_part.as_str(), true)?
        } else {
            return Err(Error::BadBunkerUrl);
        };

        if let Some(param_part) = captures.get(2) {
            let assignments = param_part.as_str().split('&');
            for assignment in assignments {
                let halfs: Vec<&str> = assignment.split('=').collect();
                if halfs.len() != 2 {
                    return Err(Error::BadBunkerUrl);
                }
                let var = halfs[0];
                let val = halfs[1];
                match var {
                    "relay" => relays.push(RelayUrl::try_from_str(val)?),
                    "secret" => secret = Some(val.to_owned()),
                    _ => continue, // ignore other terms
                }
            }
        } else {
            return Err(Error::BadBunkerUrl);
        }

        if relays.is_empty() {
            return Err(Error::BadBunkerUrl);
        }

        Ok(Nip46ConnectionParameters {
            remote_signer_pubkey: public_key,
            relays,
            secret,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_nip46_connection_params() {
        let params = Nip46ConnectionParameters::from_str(
            "bunker://ee11a5dff40c19a555f41fe42b48f00e618c91225622ae37b6c2bb67b76c4e49?relay=wss://chorus.mikedilger.com:444/&secret=5ijGGB0AGmgAAAAAgbaYUxymvgMQnrQh"
        ).unwrap();

        assert_eq!(params.relays.len(), 1);
        assert_eq!(
            params.relays[0].as_str(),
            "wss://chorus.mikedilger.com:444/"
        );
        assert_eq!(
            params.remote_signer_pubkey,
            PublicKey::try_from_hex_string(
                "ee11a5dff40c19a555f41fe42b48f00e618c91225622ae37b6c2bb67b76c4e49",
                true
            )
            .unwrap()
        );
        assert_eq!(
            params.secret,
            Some("5ijGGB0AGmgAAAAAgbaYUxymvgMQnrQh".to_string())
        );
    }
}
