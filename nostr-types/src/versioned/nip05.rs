use crate::types::{PublicKeyHex, UncheckedUrl};
use serde::{Deserialize, Serialize};
#[cfg(feature = "speedy")]
use speedy::{Readable, Writable};
use std::collections::HashMap;

/// The content of a webserver's /.well-known/nostr.json file used in NIP-05 and NIP-35
/// This allows lookup and verification of a nostr user via a `user@domain` style identifier.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct Nip05V1 {
    /// DNS names mapped to public keys
    pub names: HashMap<String, PublicKeyHex>,

    /// Public keys mapped to arrays of relays where they post
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(default)]
    pub relays: HashMap<PublicKeyHex, Vec<UncheckedUrl>>,
}

impl Nip05V1 {
    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) fn mock() -> Nip05V1 {
        let pubkey = PublicKeyHex::try_from_str(
            "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9",
        )
        .unwrap();

        let mut names: HashMap<String, PublicKeyHex> = HashMap::new();
        let _ = names.insert("bob".to_string(), pubkey.clone());

        let mut relays: HashMap<PublicKeyHex, Vec<UncheckedUrl>> = HashMap::new();
        let _ = relays.insert(
            pubkey,
            vec![
                UncheckedUrl::from_str("wss://relay.example.com"),
                UncheckedUrl::from_str("wss://relay2.example.com"),
            ],
        );

        Nip05V1 { names, relays }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    test_serde! {Nip05V1, test_nip05_serde}

    #[test]
    fn test_nip05_example() {
        let body = r#"{
  "names": {
    "bob": "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9"
  },
  "relays": {
    "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9": [ "wss://relay.example.com", "wss://relay2.example.com" ]
  }
}"#;

        let nip05: Nip05V1 = serde_json::from_str(body).unwrap();

        let bobs_pk: PublicKeyHex = nip05.names.get("bob").unwrap().clone();
        assert_eq!(
            bobs_pk.as_str(),
            "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9"
        );

        let bobs_relays: Vec<UncheckedUrl> = nip05.relays.get(&bobs_pk).unwrap().to_owned();

        assert_eq!(
            bobs_relays,
            vec![
                UncheckedUrl::from_str("wss://relay.example.com"),
                UncheckedUrl::from_str("wss://relay2.example.com")
            ]
        );
    }
}
