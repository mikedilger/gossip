use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, Serializer};
use serde_json::{json, Map, Value};
use std::fmt;

/// Metadata about a user
///
/// Note: the value is an Option because some real-world data has been found to
/// contain JSON nulls as values, and we don't want deserialization of those
/// events to fail. We treat these in our get() function the same as if the key
/// did not exist.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataV1 {
    /// username
    pub name: Option<String>,

    /// about
    pub about: Option<String>,

    /// picture URL
    pub picture: Option<String>,

    /// nip05 dns id
    pub nip05: Option<String>,

    /// Additional fields not specified in NIP-01 or NIP-05
    pub other: Map<String, Value>,
}

impl Default for MetadataV1 {
    fn default() -> Self {
        MetadataV1 {
            name: None,
            about: None,
            picture: None,
            nip05: None,
            other: Map::new(),
        }
    }
}

impl MetadataV1 {
    /// Create new empty Metadata
    pub fn new() -> MetadataV1 {
        MetadataV1::default()
    }

    #[allow(dead_code)]
    pub(crate) fn mock() -> MetadataV1 {
        let mut map = Map::new();
        let _ = map.insert(
            "display_name".to_string(),
            Value::String("William Caserin".to_string()),
        );
        MetadataV1 {
            name: Some("jb55".to_owned()),
            about: None,
            picture: None,
            nip05: Some("jb55.com".to_owned()),
            other: map,
        }
    }

    /// Get the lnurl for the user, if available via lud06 or lud16
    pub fn lnurl(&self) -> Option<String> {
        if let Some(Value::String(lud06)) = self.other.get("lud06") {
            if let Ok(data) = bech32::decode(lud06) {
                if data.0 == *crate::HRP_LNURL {
                    return Some(String::from_utf8_lossy(&data.1).to_string());
                }
            }
        }

        if let Some(Value::String(lud16)) = self.other.get("lud16") {
            let vec: Vec<&str> = lud16.split('@').collect();
            if vec.len() == 2 {
                let user = &vec[0];
                let domain = &vec[1];
                return Some(format!("https://{domain}/.well-known/lnurlp/{user}"));
            }
        }

        None
    }
}

impl Serialize for MetadataV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(4 + self.other.len()))?;
        map.serialize_entry("name", &json!(&self.name))?;
        map.serialize_entry("about", &json!(&self.about))?;
        map.serialize_entry("picture", &json!(&self.picture))?;
        map.serialize_entry("nip05", &json!(&self.nip05))?;
        for (k, v) in &self.other {
            map.serialize_entry(&k, &v)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for MetadataV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(MetadataV1Visitor)
    }
}

struct MetadataV1Visitor;

impl<'de> Visitor<'de> for MetadataV1Visitor {
    type Value = MetadataV1;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "A JSON object")
    }

    fn visit_map<M>(self, mut access: M) -> Result<MetadataV1, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut map: Map<String, Value> = Map::new();
        while let Some((key, value)) = access.next_entry::<String, Value>()? {
            let _ = map.insert(key, value);
        }

        let mut m: MetadataV1 = Default::default();

        if let Some(Value::String(s)) = map.remove("name") {
            m.name = Some(s);
        }
        if let Some(Value::String(s)) = map.remove("about") {
            m.about = Some(s);
        }
        if let Some(Value::String(s)) = map.remove("picture") {
            m.picture = Some(s);
        }
        if let Some(Value::String(s)) = map.remove("nip05") {
            m.nip05 = Some(s);
        }

        m.other = map;

        Ok(m)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    test_serde! {MetadataV1, test_metadata_serde}

    #[test]
    fn test_metadata_print_json() {
        // I want to see if JSON serialized metadata is network appropriate
        let m = MetadataV1::mock();
        println!("{}", serde_json::to_string(&m).unwrap());
    }

    #[test]
    fn test_tolerate_nulls() {
        let json = r##"{"name":"monlovesmango","picture":"https://astral.ninja/aura/monlovesmango.svg","about":"building on nostr","nip05":"monlovesmango@astral.ninja","lud06":null,"testing":"123"}"##;
        let m: MetadataV1 = serde_json::from_str(json).unwrap();
        assert_eq!(m.name, Some("monlovesmango".to_owned()));
        assert_eq!(m.other.get("lud06"), Some(&Value::Null));
        assert_eq!(
            m.other.get("testing"),
            Some(&Value::String("123".to_owned()))
        );
    }

    #[test]
    fn test_metadata_lnurls() {
        // test lud06
        let json = r##"{"name":"mikedilger","about":"Author of Gossip client: https://github.com/mikedilger/gossip\nexpat American living in New Zealand","picture":"https://avatars.githubusercontent.com/u/1669069","nip05":"_@mikedilger.com","banner":"https://mikedilger.com/banner.jpg","display_name":"Michael Dilger","location":"New Zealand","lud06":"lnurl1dp68gurn8ghj7ampd3kx2ar0veekzar0wd5xjtnrdakj7tnhv4kxctttdehhwm30d3h82unvwqhkgetrv4h8gcn4dccnxv563ep","website":"https://mikedilger.com"}"##;
        let m: MetadataV1 = serde_json::from_str(json).unwrap();
        assert_eq!(
            m.lnurl().as_deref(),
            Some("https://walletofsatoshi.com/.well-known/lnurlp/decentbun13")
        );

        // test lud16
        let json = r##"{"name":"mikedilger","about":"Author of Gossip client: https://github.com/mikedilger/gossip\nexpat American living in New Zealand","picture":"https://avatars.githubusercontent.com/u/1669069","nip05":"_@mikedilger.com","banner":"https://mikedilger.com/banner.jpg","display_name":"Michael Dilger","location":"New Zealand","lud16":"decentbun13@walletofsatoshi.com","website":"https://mikedilger.com"}"##;
        let m: MetadataV1 = serde_json::from_str(json).unwrap();
        assert_eq!(
            m.lnurl().as_deref(),
            Some("https://walletofsatoshi.com/.well-known/lnurlp/decentbun13")
        );
    }
}
