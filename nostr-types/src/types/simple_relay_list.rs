use crate::types::UncheckedUrl;
use serde::de::{Deserializer, MapAccess, Visitor};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
#[cfg(feature = "speedy")]
use speedy::{Readable, Writable};
use std::collections::HashMap;
use std::fmt;

/// When and how to use a Relay
///
/// This is used only for `SimpleRelayList`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct SimpleRelayUsage {
    /// Whether to write to this relay
    pub write: bool,

    /// Whether to read from this relay
    pub read: bool,
}

impl Default for SimpleRelayUsage {
    fn default() -> SimpleRelayUsage {
        SimpleRelayUsage {
            write: false,
            read: true,
        }
    }
}

/// A list of relays with SimpleRelayUsage
///
/// This is only used for handling the contents of a kind-3 contact list.
/// For normal relay lists, consider using `RelayList` instead.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct SimpleRelayList(pub HashMap<UncheckedUrl, SimpleRelayUsage>);

impl SimpleRelayList {
    #[allow(dead_code)]
    pub(crate) fn mock() -> SimpleRelayList {
        let mut map: HashMap<UncheckedUrl, SimpleRelayUsage> = HashMap::new();
        let _ = map.insert(
            UncheckedUrl::from_str("wss://nostr.oxtr.dev"),
            SimpleRelayUsage {
                write: true,
                read: true,
            },
        );
        let _ = map.insert(
            UncheckedUrl::from_str("wss://nostr-relay.wlvs.space"),
            SimpleRelayUsage {
                write: false,
                read: true,
            },
        );
        SimpleRelayList(map)
    }
}

impl Serialize for SimpleRelayList {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            map.serialize_entry(&k, &v)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for SimpleRelayList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(SimpleRelayListVisitor)
    }
}

struct SimpleRelayListVisitor;

impl<'de> Visitor<'de> for SimpleRelayListVisitor {
    type Value = SimpleRelayList;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "A JSON object")
    }

    fn visit_map<M>(self, mut access: M) -> Result<SimpleRelayList, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut map: HashMap<UncheckedUrl, SimpleRelayUsage> = HashMap::new();
        while let Some((key, value)) = access.next_entry::<UncheckedUrl, SimpleRelayUsage>()? {
            let _ = map.insert(key, value);
        }
        Ok(SimpleRelayList(map))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    test_serde! {SimpleRelayList, test_simple_relay_list_serde}

    #[test]
    fn test_simple_relay_list_json() {
        let serialized = r#"{"wss://nostr.oxtr.dev":{"write":true,"read":true},"wss://relay.damus.io":{"write":true,"read":true},"wss://nostr.fmt.wiz.biz":{"write":true,"read":true},"wss://nostr-relay.wlvs.space":{"write":true,"read":true}}"#;
        let _simple_relay_list: SimpleRelayList = serde_json::from_str(serialized).unwrap();
    }
}
