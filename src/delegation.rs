use nostr_types::{PublicKeyHex, PublicKey, Signature, SignatureHex, Tag};
use serde_json::json;

// Returns parsed tag & delegator pub; error is string (for simplicity)
// TODO should come from nostr-types
pub(crate) fn parse_delegation_tag(tag: &str) -> Result<(Tag, PublicKey), String> {
    // TODO parsing should be done using nostr crate v0.19 DelegationTag
    match json::parse(tag) {
        Err(e) => return Err(format!("Could not parse tag, {}", e.to_string())),
        Ok(jv) => {
            if !jv.is_array() || jv.len() < 4 {
                return Err(format!("Expected array with 4 elements"));
            }
            if !jv[0].is_string() || !jv[1].is_string() || !jv[2].is_string() || !jv[3].is_string()
            {
                return Err(format!("Expected array with 4 strings"));
            }
            if jv[0].as_str().unwrap() != "delegation" {
                return Err(format!("First string should be 'delegation'"));
            }
            match PublicKey::try_from_hex_string(jv[1].as_str().unwrap()) {
                Err(e) => return Err(format!("Could not parse public key, {}", e.to_string())),
                Ok(public_key) => {
                    let pubkey = PublicKeyHex::from(public_key);
                    let conditions = jv[2].as_str().unwrap().to_string();
                    let sig_str = jv[3].as_str().unwrap();
                    match Signature::try_from_hex_string(sig_str) {
                        Err(e) => {
                            return Err(format!("Could not parse signature, {}", e.to_string()))
                        }
                        Ok(signature) => {
                            let sig = SignatureHex::from(signature);
                            Ok((
                                Tag::Delegation {
                                    pubkey,
                                    conditions,
                                    sig,
                                },
                                public_key,
                            ))
                        }
                    }
                }
            }
        }
    }
}

/// Serialize a delegation tag into JSON string
// TODO should come from nostr-types
pub(crate) fn serialize_delegation_tag(tag: &Tag) -> String {
    match tag {
        Tag::Delegation {
            pubkey,
            conditions,
            sig,
        } => json!(["delegation", pubkey.as_str(), conditions, sig.to_string(),]).to_string(),
        _ => "".to_string(),
    }
}
