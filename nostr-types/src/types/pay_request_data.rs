use super::{PublicKeyHex, UncheckedUrl};
use serde::de::Error as DeError;
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, Serializer};
use serde_json::{json, Map, Value};
use std::fmt;

/// This is a response from a zapper lnurl
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayRequestData {
    /// The URL to make the pay request to with a kind 9374 event
    pub callback: UncheckedUrl,

    /// Metadata
    pub metadata: Vec<(String, String)>,

    /// Whether the lnurl supports nostr zaps
    pub allows_nostr: Option<bool>,

    /// The nostr public key of the zapper
    pub nostr_pubkey: Option<PublicKeyHex>,

    /// Other fields such as:
    ///
    /// "maxSendable": 100000000000,
    /// "minSendable": 1000,
    /// "commentAllowed": 32
    /// "tag": "payRequest"
    pub other: Map<String, Value>,
}

impl Default for PayRequestData {
    fn default() -> Self {
        PayRequestData {
            callback: UncheckedUrl("".to_owned()),
            metadata: vec![],
            allows_nostr: None,
            nostr_pubkey: None,
            other: Map::new(),
        }
    }
}

impl PayRequestData {
    #[allow(dead_code)]
    pub(crate) fn mock() -> PayRequestData {
        let mut map = Map::new();
        let _ = map.insert("tag".to_string(), Value::String("payRequest".to_owned()));
        let _ = map.insert(
            "maxSendable".to_string(),
            Value::Number(100000000000_u64.into()),
        );
        let _ = map.insert("minSendable".to_string(), Value::Number(1000.into()));
        let _ = map.insert("commentAllowed".to_string(), Value::Number(32.into()));
        PayRequestData {
            callback: UncheckedUrl("https://livingroomofsatoshi.com/api/v1/lnurl/payreq/f16bacaa-8e5f-4038-bdea-4c9e796f913c".to_string()),
            metadata: vec![
                ("text/plain".to_owned(),
                 "Pay to Wallet of Satoshi user: decentbun13".to_owned()),
                ("text/identifier".to_owned(),
                 "decentbun13@walletofsatoshi.com".to_owned()),
            ],
            allows_nostr: Some(true),
            nostr_pubkey: Some(PublicKeyHex::try_from_str("be1d89794bf92de5dd64c1e60f6a2c70c140abac9932418fee30c5c637fe9479").unwrap()),
            other: map,
        }
    }
}

impl Serialize for PayRequestData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(4 + self.other.len()))?;
        map.serialize_entry("callback", &json!(&self.callback))?;
        map.serialize_entry("metadata", &json!(&self.metadata))?;
        map.serialize_entry("allowsNostr", &json!(&self.allows_nostr))?;
        map.serialize_entry("nostrPubkey", &json!(&self.nostr_pubkey))?;
        for (k, v) in &self.other {
            map.serialize_entry(&k, &v)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for PayRequestData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(PayRequestDataVisitor)
    }
}

struct PayRequestDataVisitor;

impl<'de> Visitor<'de> for PayRequestDataVisitor {
    type Value = PayRequestData;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "A JSON object")
    }

    fn visit_map<M>(self, mut access: M) -> Result<PayRequestData, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut map: Map<String, Value> = Map::new();
        while let Some((key, value)) = access.next_entry::<String, Value>()? {
            let _ = map.insert(key, value);
        }

        let mut m: PayRequestData = Default::default();

        if let Some(Value::String(s)) = map.remove("callback") {
            m.callback = UncheckedUrl(s)
        } else {
            return Err(DeError::custom("Missing callback url".to_owned()));
        }

        if let Some(Value::Array(a)) = map.remove("metadata") {
            for elem in a.iter() {
                if let Value::Array(a2) = elem {
                    if a2.len() == 2 {
                        if let Value::String(key) = &a2[0] {
                            if let Value::String(val) = &a2[1] {
                                m.metadata.push((key.to_owned(), val.to_owned()));
                            }
                        }
                    } else {
                        return Err(DeError::custom("Metadata entry not a pair".to_owned()));
                    }
                } else {
                    return Err(DeError::custom("Metadata entry not recognized".to_owned()));
                }
            }
        }

        if let Some(Value::Bool(b)) = map.remove("allowsNostr") {
            m.allows_nostr = Some(b);
        } else {
            m.allows_nostr = None;
        }

        if let Some(Value::String(s)) = map.remove("nostrPubkey") {
            m.nostr_pubkey = match PublicKeyHex::try_from_string(s) {
                Ok(pkh) => Some(pkh),
                Err(e) => return Err(DeError::custom(format!("{e}"))),
            };
        }

        m.other = map;

        Ok(m)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    test_serde! {PayRequestData, test_pay_request_data_serde}
}
