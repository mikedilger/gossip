use crate::types::{Event, ParsedTag, RelayUrl, Tag};
use std::collections::HashMap;

/// Relay Usage
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RelayListUsage {
    /// The relay is used as an inbox (called 'read' in kind-10002)
    Inbox,

    /// The relay is used as an outbox (called 'write' in kind-10002)
    Outbox,

    /// The relay is used both as an inbox and an outbox
    #[default]
    Both,
}

impl RelayListUsage {
    /// A string marker used in a kind-10002 RelayList event for the variant
    pub fn marker(&self) -> Option<&'static str> {
        match self {
            RelayListUsage::Inbox => Some("read"),
            RelayListUsage::Outbox => Some("write"),
            RelayListUsage::Both => None,
        }
    }
}

/// A relay list, indicating usage for each relay, which can be used to
/// represent the data found in a kind 10002 RelayListMetadata event.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RelayList(pub HashMap<RelayUrl, RelayListUsage>);

impl RelayList {
    /// Parse a kind-10002 RelayList event into a RelayList
    ///
    /// This does not check the event kind, that is left up to the caller.
    pub fn from_event(event: &Event) -> RelayList {
        let mut relay_list: RelayList = Default::default();

        for tag in event.tags.iter() {
            if let Ok(ParsedTag::RelayUsage { url, usage }) = tag.parse() {
                if let Ok(relay_url) = RelayUrl::try_from_unchecked_url(&url) {
                    if let Some(m) = usage {
                        match &*m.trim().to_lowercase() {
                            "read" => {
                                let _ = relay_list.0.insert(relay_url, RelayListUsage::Inbox);
                            }
                            "write" => {
                                let _ = relay_list.0.insert(relay_url, RelayListUsage::Outbox);
                            }
                            _ => {} // ignore unknown marker
                        }
                    } else {
                        let _ = relay_list.0.insert(relay_url, RelayListUsage::Both);
                    }
                }
            }
        }

        relay_list
    }

    /// Create a `Vec<Tag>` appropriate for forming a kind-10002 RelayList event
    pub fn to_event_tags(&self) -> Vec<Tag> {
        let mut tags: Vec<Tag> = Vec::new();
        for (relay_url, usage) in self.0.iter() {
            tags.push(
                ParsedTag::RelayUsage {
                    url: relay_url.to_unchecked_url(),
                    usage: usage.marker().map(|s| s.to_owned()),
                }
                .into_tag(),
            );
        }
        tags
    }
}
