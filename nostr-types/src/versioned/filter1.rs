use crate::Error;
use crate::{Event, EventKind, Id, IdHex, PublicKey, PublicKeyHex, Tag, Unixtime};
use serde::de::{Deserializer, MapAccess, Visitor};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
#[cfg(feature = "speedy")]
use speedy::{Readable, Writable};
use std::collections::BTreeMap;
use std::fmt;

/// Filter which specify what events a client is looking for
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct FilterV1 {
    /// Events which match these ids
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub ids: Vec<IdHex>, // ID as hex

    /// Events which match these authors
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub authors: Vec<PublicKeyHex>, // PublicKey as hex

    /// Events which match these kinds
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub kinds: Vec<EventKind>,

    /// Events which match the given tags
    #[serde(
        flatten,
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub tags: BTreeMap<char, Vec<String>>,

    /// Events occuring after this date
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub since: Option<Unixtime>,

    /// Events occuring before this date
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub until: Option<Unixtime>,

    /// A limit on the number of events to return in the initial query
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub limit: Option<usize>,
}

impl FilterV1 {
    /// Create a new Filter object
    pub fn new() -> FilterV1 {
        Default::default()
    }

    /// Add an Id to the filter.
    pub fn add_id(&mut self, id: Id) {
        let idhex: IdHex = id.into();
        if !self.ids.contains(&idhex) {
            self.ids.push(idhex);
        }
    }

    /// Delete an Id from the filter
    pub fn del_id(&mut self, id: Id) {
        let idhex: IdHex = id.into();
        if let Some(index) = self.ids.iter().position(|id| *id == idhex) {
            let _ = self.ids.swap_remove(index);
        }
    }

    /// Add a PublicKey to the filter
    pub fn add_author(&mut self, public_key: PublicKey) {
        let pkh: PublicKeyHex = public_key.into();
        if !self.authors.contains(&pkh) {
            self.authors.push(pkh);
        }
    }

    /// Delete a PublicKey from the filter
    pub fn del_author(&mut self, public_key: PublicKey) {
        let pkh: PublicKeyHex = public_key.into();
        if let Some(index) = self.authors.iter().position(|pk| *pk == pkh) {
            let _ = self.authors.swap_remove(index);
        }
    }

    /// Add an EventKind to the filter
    pub fn add_event_kind(&mut self, event_kind: EventKind) {
        if self.kinds.contains(&event_kind) {
            return;
        }
        self.kinds.push(event_kind);
    }

    /// Delete an EventKind from the filter
    pub fn del_event_kind(&mut self, event_kind: EventKind) {
        if let Some(position) = self.kinds.iter().position(|&x| x == event_kind) {
            let _ = self.kinds.swap_remove(position);
        }
    }

    /// Add a Tag value to a filter
    pub fn add_tag_value(&mut self, letter: char, value: String) {
        let _ = self
            .tags
            .entry(letter)
            .and_modify(|values| values.push(value.clone()))
            .or_insert(vec![value]);
    }

    /// Add a Tag value from a filter
    pub fn del_tag_value(&mut self, letter: char, value: String) {
        let mut became_empty: bool = false;
        let _ = self.tags.entry(letter).and_modify(|values| {
            if let Some(position) = values.iter().position(|x| *x == value) {
                let _ = values.swap_remove(position);
            }
            if values.is_empty() {
                became_empty = true;
            }
        });
        if became_empty {
            let _ = self.tags.remove(&letter);
        }
    }

    /// Set all values for a given tag
    pub fn set_tag_values(&mut self, letter: char, values: Vec<String>) {
        let _ = self.tags.insert(letter, values);
    }

    /// Remove all Tag values of a given kind from a filter
    pub fn clear_tag_values(&mut self, letter: char) {
        let _ = self.tags.remove(&letter);
    }

    /// Convert filter tags into a `Vec<Tag>`
    pub fn tags_as_tags(&self) -> Vec<Tag> {
        let mut buffer: [u8; 4] = [0; 4];
        let mut tags: Vec<Tag> = Vec::with_capacity(self.tags.len());
        for (letter, values) in self.tags.iter() {
            let mut strings: Vec<String> = Vec::with_capacity(1 + values.len());
            strings.push(letter.encode_utf8(&mut buffer).to_owned());
            strings.extend(values.to_owned());
            tags.push(Tag::from_strings(strings));
        }
        tags
    }

    /// Does the event match the filter?
    pub fn event_matches(&self, e: &Event) -> bool {
        if !self.ids.is_empty() {
            let idhex: IdHex = e.id.into();
            if !self.ids.contains(&idhex) {
                return false;
            }
        }

        if !self.authors.is_empty() {
            let pubkeyhex: PublicKeyHex = e.pubkey.into();
            if !self.authors.contains(&pubkeyhex) {
                return false;
            }
        }

        if !self.kinds.is_empty() && !self.kinds.contains(&e.kind) {
            return false;
        }

        if let Some(since) = self.since {
            if e.created_at < since {
                return false;
            }
        }

        if let Some(until) = self.until {
            if e.created_at > until {
                return false;
            }
        }

        'tags: for (letter, values) in &self.tags {
            for tag in &e.tags {
                if tag.tagname().starts_with(*letter) && values.iter().any(|v| v == tag.value()) {
                    continue 'tags;
                }
            }

            return false;
        }

        true
    }

    /// The offset used for NIP-45 (pr #1561)
    pub fn hyperloglog_offset(&self) -> Result<usize, Error> {
        let r = |vec: &Vec<String>| {
            let bytes = match hex::decode(&vec[0].as_bytes()[32..=33]) {
                Ok(b) => b,
                Err(_) => return Ok(16),
            };
            Ok(bytes[0] as usize % 24)
        };

        if let Some(vec) = self.tags.get(&'e') {
            if vec.len() == 1 {
                return r(vec);
            }
        }

        if let Some(vec) = self.tags.get(&'p') {
            if vec.len() == 1 {
                return r(vec);
            }
        }

        if let Some(vec) = self.tags.get(&'a') {
            if vec.len() == 1 {
                return r(vec);
            }
        }

        if let Some(vec) = self.tags.get(&'q') {
            if vec.len() == 1 {
                return r(vec);
            }
        }

        Ok(16)
    }

    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) fn mock() -> FilterV1 {
        let mut map = BTreeMap::new();
        let _ = map.insert('e', vec![IdHex::mock().to_string()]);
        let _ = map.insert(
            'p',
            vec!["221115830ced1ca94352002485fcc7a75dcfe30d1b07f5f6fbe9c0407cfa59a1".to_string()],
        );

        FilterV1 {
            ids: vec![IdHex::try_from_str(
                "3ab7b776cb547707a7497f209be799710ce7eb0801e13fd3c4e7b9261ac29084",
            )
            .unwrap()],
            authors: vec![],
            kinds: vec![EventKind::TextNote, EventKind::Metadata],
            tags: map,
            since: Some(Unixtime(1668572286)),
            ..Default::default()
        }
    }
}

fn serialize_tags<S>(tags: &BTreeMap<char, Vec<String>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut map = serializer.serialize_map(Some(tags.len()))?;
    for (tag, values) in tags.iter() {
        map.serialize_entry(&format!("#{tag}"), values)?;
    }
    map.end()
}

fn deserialize_tags<'de, D>(deserializer: D) -> Result<BTreeMap<char, Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct TagsVisitor;

    impl<'de> Visitor<'de> for TagsVisitor {
        type Value = BTreeMap<char, Vec<String>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("map with keys in \"#t\" format")
        }

        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            let mut tags: BTreeMap<char, Vec<String>> = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, Vec<String>>()? {
                let mut chars = key.chars();
                if let (Some('#'), Some(ch), None) = (chars.next(), chars.next(), chars.next()) {
                    let _ = tags.insert(ch, value);
                }
            }
            Ok(tags)
        }
    }

    deserializer.deserialize_map(TagsVisitor)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ParsedTag;

    test_serde! {FilterV1, test_filters_serde}

    #[test]
    fn test_filter_mock() {
        assert_eq!(
            &serde_json::to_string(&FilterV1::mock()).unwrap(),
            r##"{"ids":["3ab7b776cb547707a7497f209be799710ce7eb0801e13fd3c4e7b9261ac29084"],"kinds":[1,0],"#e":["5df64b33303d62afc799bdc36d178c07b2e1f0d824f31b7dc812219440affab6"],"#p":["221115830ced1ca94352002485fcc7a75dcfe30d1b07f5f6fbe9c0407cfa59a1"],"since":1668572286}"##
        );
    }

    #[test]
    fn test_filter_example() {
        let raw_event = r##"{"id":"dcf0f0339a9868fc5f51867f27049186fd8497816a19967ba4f03a3edf65a647","pubkey":"f647c9568d09596e323fdd0144b8e2f35aaf5daa43f9eb59b502e99d90f43673","created_at":1715996970,"kind":7,"sig":"ef592a256107217d6710ba18f8446494f0feb99df430284d1d5a36e7859b04e642f40746916ecffb98614d57d9053242c543ad05a74f2c5843dc9ba2169c5175","content":"🫂","tags":[["e","b74444aaaee395e4e76de1902d00457742eecefbd6ee329a79a6ae125f97fcbf"],["p","a723805cda67251191c8786f4da58f797e6977582301354ba8e91bcb0342dc9c"],["k","1"]]}"##;
        let event: Event = serde_json::from_str(raw_event).unwrap();

        let mut filter = FilterV1 {
            kinds: vec![EventKind::Reaction],
            ..Default::default()
        };
        filter.set_tag_values(
            'p',
            vec!["a723805cda67251191c8786f4da58f797e6977582301354ba8e91bcb0342dc9c".to_owned()],
        );

        assert!(filter.event_matches(&event));
    }

    #[test]
    fn test_add_remove_id() {
        let mock = Id::mock();

        let mut filters: FilterV1 = FilterV1::new();

        filters.add_id(mock);
        assert_eq!(filters.ids.len(), 1);
        filters.add_id(mock); // overwrites
        assert_eq!(filters.ids.len(), 1);
        filters.del_id(mock);
        assert!(filters.ids.is_empty());
    }

    // add_remove_author would be very similar to the above

    #[test]
    fn test_add_remove_tags() {
        let mut filter = FilterV1::mock();
        filter.del_tag_value('e', IdHex::mock().to_string());
        assert_eq!(filter.tags.get(&'e'), None);

        filter.add_tag_value('t', "footstr".to_string());
        filter.add_tag_value('t', "bitcoin".to_string());
        filter.del_tag_value('t', "bitcoin".to_string());
        assert!(filter.tags.contains_key(&'t'));
    }

    #[tokio::test]
    async fn test_event_matches() {
        use crate::{Id, KeySigner, PreEvent, PrivateKey, Signer, UncheckedUrl};

        let signer = {
            let privkey = PrivateKey::mock();
            KeySigner::from_private_key(privkey, "", 1).unwrap()
        };
        let preevent = PreEvent {
            pubkey: signer.public_key(),
            created_at: Unixtime(1680000012),
            kind: EventKind::TextNote,
            tags: vec![
                ParsedTag::Event {
                    id: Id::mock(),
                    recommended_relay_url: Some(UncheckedUrl::mock()),
                    marker: None,
                    author_pubkey: None,
                }
                .into_tag(),
                ParsedTag::Hashtag("foodstr".to_string()).into_tag(),
            ],
            content: "Hello World!".to_string(),
        };
        let event = signer.sign_event(preevent).await.unwrap();

        let mut filter = FilterV1 {
            authors: vec![signer.public_key().into()],
            ..Default::default()
        };
        filter.add_tag_value('e', Id::mock().as_hex_string());
        assert!(filter.event_matches(&event));

        let filter = FilterV1 {
            authors: vec![signer.public_key().into()],
            kinds: vec![EventKind::LongFormContent],
            ..Default::default()
        };
        assert!(!filter.event_matches(&event));

        let filter = FilterV1 {
            ids: vec![IdHex::mock()],
            authors: vec![signer.public_key().into()],
            ..Default::default()
        };
        assert!(!filter.event_matches(&event));
    }
}
