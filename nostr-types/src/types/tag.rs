use crate::versioned::tag3::TagV3;
use crate::{
    DelegationConditions, Error, EventKind, EventReference, Id, IntoVec, NAddr, PublicKey,
    RelayUrl, Signature, UncheckedUrl,
};

/// A tag on an Event
pub type Tag = TagV3;

/// This parses known simple tags in a uniform way.
///
/// This is incomplete and because it is kind-independent you should beware as
/// some tags are reused for different purposes in different kinds.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum ParsedTag {
    Address {
        address: NAddr,
        marker: Option<String>,
    },
    ContentWarning(Option<String>),
    Delegation {
        pubkey: PublicKey,
        conditions: DelegationConditions,
        sig: Signature,
    },
    Event {
        id: Id,
        recommended_relay_url: Option<UncheckedUrl>,
        marker: Option<String>,
        author_pubkey: Option<PublicKey>,
    },
    Hashtag(String),
    Identifier(String),
    Kind(EventKind),
    Nonce {
        nonce: u32,
        target: Option<u32>,
    },
    Proxy {
        id: String,
        protocol: String,
    },
    Pubkey {
        pubkey: PublicKey,
        recommended_relay_url: Option<UncheckedUrl>,
        petname: Option<String>,
    },
    Quote {
        id: Id,
        recommended_relay_url: Option<UncheckedUrl>,
        author_pubkey: Option<PublicKey>,
    },
    RelayUsage {
        url: UncheckedUrl,
        usage: Option<String>,
    },
    RootAddress {
        address: NAddr,
        marker: Option<String>,
    },
    RootEvent {
        id: Id,
        recommended_relay_url: Option<UncheckedUrl>,
        marker: Option<String>,
        author_pubkey: Option<PublicKey>,
    },
    RootKind(EventKind),
    RootPubkey {
        pubkey: PublicKey,
        recommended_relay_url: Option<UncheckedUrl>,
        petname: Option<String>,
    },
    Subject(String),
    Summary(String),
    Title(String),
    Unmatched(Vec<String>),
}

impl ParsedTag {
    /// Attempt to parse a tag.  Will yield a `Parsed::Unmatched` on any tag
    /// that we haven't implemented.
    ///
    /// # Errors
    ///
    /// Will yield an `Err` if the data of the tag is inappropriate.
    pub fn parse(tag: &Tag) -> Result<ParsedTag, Error> {
        match tag.tagname() {
            "a" => {
                let (kind, author, d) = {
                    let parts: Vec<&str> = tag
                        .get_opt_index(1)
                        .ok_or(Error::TagMismatch)?
                        .split(':')
                        .collect();
                    if parts.len() < 3 {
                        return Err(Error::TagMismatch);
                    }
                    let kind: EventKind = {
                        let kindnum: u32 = parts[0].parse::<u32>()?;
                        From::from(kindnum)
                    };
                    if !kind.is_replaceable() {
                        return Err(Error::NonReplaceableAddr);
                    }
                    let author: PublicKey = PublicKey::try_from_hex_string(parts[1], true)?;
                    let d = parts[2].to_string();
                    (kind, author, d)
                };

                let url = tag.get_opt_index(2).map(|s| UncheckedUrl(s.to_owned()));
                let relays = match url {
                    None => Vec::new(),
                    Some(r) => vec![r],
                };

                let na = NAddr {
                    d,
                    relays,
                    kind,
                    author,
                };

                Ok(ParsedTag::Address {
                    address: na,
                    marker: tag.get_opt_index(3).map(|x| x.to_string()),
                })
            }
            "content-warning" => Ok(ParsedTag::ContentWarning(
                tag.get_opt_index(1).map(|x| x.to_string()),
            )),
            "delegation" => {
                let pubkey = PublicKey::try_from_hex_string(
                    tag.get_opt_index(1).ok_or(Error::TagMismatch)?,
                    true,
                )?;
                let conditions = DelegationConditions::try_from_str(
                    tag.get_opt_index(2).ok_or(Error::TagMismatch)?,
                )?;
                let sig = Signature::try_from_hex_string(
                    tag.get_opt_index(3).ok_or(Error::TagMismatch)?,
                )?;
                Ok(ParsedTag::Delegation {
                    pubkey,
                    conditions,
                    sig,
                })
            }
            "e" => {
                let id = Id::try_from_hex_string(tag.get_opt_index(1).ok_or(Error::TagMismatch)?)?;
                let recommended_relay_url =
                    tag.get_opt_index(2).map(|s| UncheckedUrl(s.to_string()));
                let marker = tag.get_opt_index(3).map(|m| m.to_string());
                let author_pubkey = match tag.get_opt_index(4) {
                    None => None,
                    Some(h) => Some(PublicKey::try_from_hex_string(h, true)?),
                };
                Ok(ParsedTag::Event {
                    id,
                    recommended_relay_url,
                    marker,
                    author_pubkey,
                })
            }
            "t" => Ok(ParsedTag::Hashtag(
                tag.get_opt_index(1).ok_or(Error::TagMismatch)?.to_string(),
            )),
            "d" => Ok(ParsedTag::Identifier(
                tag.get_opt_index(1).ok_or(Error::TagMismatch)?.to_string(),
            )),
            "k" => Ok(ParsedTag::Kind(
                tag.get_opt_index(1)
                    .ok_or(Error::TagMismatch)?
                    .parse::<u32>()?
                    .into(),
            )),
            "nonce" => {
                let nonce = tag
                    .get_opt_index(1)
                    .ok_or(Error::TagMismatch)?
                    .parse::<u32>()?;
                let target = match tag.get_opt_index(2) {
                    None => None,
                    Some(s) => Some(s.parse::<u32>()?),
                };
                Ok(ParsedTag::Nonce { nonce, target })
            }
            "proxy" => Ok(ParsedTag::Proxy {
                id: tag.get_opt_index(1).ok_or(Error::TagMismatch)?.to_string(),
                protocol: tag.get_opt_index(2).ok_or(Error::TagMismatch)?.to_string(),
            }),
            "p" => {
                let pubkey = PublicKey::try_from_hex_string(
                    tag.get_opt_index(1).ok_or(Error::TagMismatch)?,
                    true,
                )?;
                let recommended_relay_url =
                    tag.get_opt_index(2).map(|s| UncheckedUrl(s.to_string()));
                let petname = tag.get_opt_index(3).map(|m| m.to_string());
                Ok(ParsedTag::Pubkey {
                    pubkey,
                    recommended_relay_url,
                    petname,
                })
            }
            "q" => {
                let id = Id::try_from_hex_string(tag.get_opt_index(1).ok_or(Error::TagMismatch)?)?;
                let recommended_relay_url =
                    tag.get_opt_index(2).map(|s| UncheckedUrl(s.to_string()));
                let author_pubkey = match tag.get_opt_index(3) {
                    None => None,
                    Some(h) => Some(PublicKey::try_from_hex_string(h, true)?),
                };
                Ok(ParsedTag::Quote {
                    id,
                    recommended_relay_url,
                    author_pubkey,
                })
            }
            "r" => {
                let url = UncheckedUrl(tag.get_opt_index(1).ok_or(Error::TagMismatch)?.to_string());
                let usage = tag.get_opt_index(2).map(|s| s.to_owned());
                Ok(ParsedTag::RelayUsage { url, usage })
            }
            "A" => {
                let (kind, author, d) = {
                    let parts: Vec<&str> = tag
                        .get_opt_index(1)
                        .ok_or(Error::TagMismatch)?
                        .split(':')
                        .collect();
                    if parts.len() < 3 {
                        return Err(Error::TagMismatch);
                    }
                    let kind: EventKind = {
                        let kindnum: u32 = parts[0].parse::<u32>()?;
                        From::from(kindnum)
                    };
                    if !kind.is_replaceable() {
                        return Err(Error::NonReplaceableAddr);
                    }
                    let author: PublicKey = PublicKey::try_from_hex_string(parts[1], true)?;
                    let d = parts[2].to_string();
                    (kind, author, d)
                };

                let url = tag.get_opt_index(2).map(|s| UncheckedUrl(s.to_owned()));
                let relays = match url {
                    None => Vec::new(),
                    Some(r) => vec![r],
                };

                let na = NAddr {
                    d,
                    relays,
                    kind,
                    author,
                };

                Ok(ParsedTag::RootAddress {
                    address: na,
                    marker: tag.get_opt_index(3).map(|x| x.to_string()),
                })
            }
            "E" => {
                let id = Id::try_from_hex_string(tag.get_opt_index(1).ok_or(Error::TagMismatch)?)?;
                let recommended_relay_url =
                    tag.get_opt_index(2).map(|s| UncheckedUrl(s.to_string()));
                let marker = tag.get_opt_index(3).map(|m| m.to_string());
                let author_pubkey = match tag.get_opt_index(4) {
                    None => None,
                    Some(h) => Some(PublicKey::try_from_hex_string(h, true)?),
                };
                Ok(ParsedTag::RootEvent {
                    id,
                    recommended_relay_url,
                    marker,
                    author_pubkey,
                })
            }
            "K" => Ok(ParsedTag::RootKind(
                tag.get_opt_index(1)
                    .ok_or(Error::TagMismatch)?
                    .parse::<u32>()?
                    .into(),
            )),
            "P" => {
                let pubkey = PublicKey::try_from_hex_string(
                    tag.get_opt_index(1).ok_or(Error::TagMismatch)?,
                    true,
                )?;
                let recommended_relay_url =
                    tag.get_opt_index(2).map(|s| UncheckedUrl(s.to_string()));
                let petname = tag.get_opt_index(3).map(|m| m.to_string());
                Ok(ParsedTag::RootPubkey {
                    pubkey,
                    recommended_relay_url,
                    petname,
                })
            }
            "subject" => Ok(ParsedTag::Subject(
                tag.get_opt_index(1).ok_or(Error::TagMismatch)?.to_string(),
            )),
            "summary" => Ok(ParsedTag::Summary(
                tag.get_opt_index(1).ok_or(Error::TagMismatch)?.to_string(),
            )),
            "title" => Ok(ParsedTag::Title(
                tag.get_opt_index(1).ok_or(Error::TagMismatch)?.to_string(),
            )),
            _ => Ok(ParsedTag::Unmatched(tag.clone().into_inner())),
        }
    }

    /// Turn a `ParsedTag` into a `Tag`
    pub fn into_tag(self) -> Tag {
        use ParsedTag::*;

        match self {
            Address { address, marker } => {
                let mut tag = Tag::from_strings(vec![
                    "a".to_owned(),
                    format!(
                        "{}:{}:{}",
                        Into::<u32>::into(address.kind),
                        address.author.as_hex_string(),
                        address.d
                    ),
                ]);
                if !address.relays.is_empty() {
                    tag.set_index(2, address.relays[0].0.clone());
                }
                if let Some(marker) = marker {
                    tag.set_index(3, marker);
                }
                tag
            }
            ContentWarning(optstr) => {
                let mut tag = Tag::new(&["content-warning"]);
                if let Some(s) = optstr {
                    tag.set_index(1, s.to_string());
                }
                tag
            }
            Delegation {
                pubkey,
                conditions,
                sig,
            } => Tag::from_strings(vec![
                "delegation".to_owned(),
                pubkey.as_hex_string(),
                conditions.as_string(),
                sig.as_hex_string(),
            ]),
            Event {
                id,
                recommended_relay_url,
                marker,
                author_pubkey,
            } => {
                let mut tag = Tag::from_strings(vec!["e".to_owned(), id.as_hex_string()]);
                if let Some(rurl) = recommended_relay_url {
                    tag.set_index(2, rurl.0);
                }
                if let Some(mark) = marker {
                    tag.set_index(3, mark);
                }
                if let Some(pk) = author_pubkey {
                    tag.set_index(4, pk.as_hex_string());
                }
                tag
            }
            Hashtag(s) => Tag::from_strings(vec!["t".to_string(), s]),
            Identifier(s) => Tag::from_strings(vec!["d".to_string(), s]),
            Kind(k) => {
                Tag::from_strings(vec!["k".to_string(), format!("{}", Into::<u32>::into(k))])
            }
            Nonce { nonce, target } => {
                let mut tag = Tag::new(&["nonce"]);
                tag.set_index(1, format!("{}", nonce));
                if let Some(targ) = target {
                    tag.set_index(2, format!("{}", targ));
                }
                tag
            }
            Proxy { id, protocol } => Tag::from_strings(vec!["proxy".to_owned(), id, protocol]),
            Pubkey {
                pubkey,
                recommended_relay_url,
                petname,
            } => {
                let mut tag = Tag::new(&["p"]);
                tag.set_index(1, pubkey.as_hex_string());
                if let Some(u) = recommended_relay_url {
                    tag.set_index(2, u.0);
                }
                if let Some(m) = petname {
                    tag.set_index(3, m);
                }
                tag
            }
            Quote {
                id,
                recommended_relay_url,
                author_pubkey,
            } => {
                let mut tag = Tag::from_strings(vec!["q".to_owned(), id.as_hex_string()]);

                if let Some(rurl) = recommended_relay_url {
                    tag.set_index(2, rurl.0);
                }

                if let Some(pk) = author_pubkey {
                    tag.set_index(3, pk.as_hex_string());
                }

                tag
            }
            RelayUsage { url, usage } => {
                let mut tag = Tag::from_strings(vec!["r".to_owned(), url.0]);
                if let Some(u) = usage {
                    tag.set_index(2, u);
                }
                tag
            }
            RootAddress { address, marker } => {
                let mut tag = Tag::from_strings(vec![
                    "A".to_owned(),
                    format!(
                        "{}:{}:{}",
                        Into::<u32>::into(address.kind),
                        address.author.as_hex_string(),
                        address.d
                    ),
                ]);
                if !address.relays.is_empty() {
                    tag.set_index(2, address.relays[0].0.clone());
                }
                if let Some(marker) = marker {
                    tag.set_index(3, marker);
                }
                tag
            }
            RootEvent {
                id,
                recommended_relay_url,
                marker,
                author_pubkey,
            } => {
                let mut tag = Tag::from_strings(vec!["E".to_owned(), id.as_hex_string()]);
                if let Some(rurl) = recommended_relay_url {
                    tag.set_index(2, rurl.0);
                }
                if let Some(mark) = marker {
                    tag.set_index(3, mark);
                }
                if let Some(pk) = author_pubkey {
                    tag.set_index(4, pk.as_hex_string());
                }
                tag
            }
            RootKind(k) => {
                Tag::from_strings(vec!["K".to_string(), format!("{}", Into::<u32>::into(k))])
            }
            RootPubkey {
                pubkey,
                recommended_relay_url,
                petname,
            } => {
                let mut tag = Tag::new(&["P"]);
                tag.set_index(1, pubkey.as_hex_string());
                if let Some(u) = recommended_relay_url {
                    tag.set_index(2, u.0);
                }
                if let Some(m) = petname {
                    tag.set_index(3, m);
                }
                tag
            }
            Subject(s) => Tag::from_strings(vec!["subject".to_string(), s]),
            Summary(s) => Tag::from_strings(vec!["summary".to_string(), s]),
            Title(s) => Tag::from_strings(vec!["title".to_string(), s]),
            Unmatched(vec) => Tag::from_strings(vec),
        }
    }

    /// Convert into an EventReference if it references an event
    pub fn into_event_reference(self) -> Option<EventReference> {
        match self {
            ParsedTag::Address { address, marker: _ } => Some(EventReference::Addr(address)),
            ParsedTag::Event {
                id,
                recommended_relay_url,
                marker,
                author_pubkey,
            } => Some(EventReference::Id {
                id,
                author: author_pubkey,
                relays: recommended_relay_url
                    .as_ref()
                    .and_then(|rru| RelayUrl::try_from_unchecked_url(rru).ok())
                    .into_vec(),
                marker,
            }),
            ParsedTag::Quote {
                id,
                recommended_relay_url,
                author_pubkey,
            } => Some(EventReference::Id {
                id,
                author: author_pubkey,
                relays: recommended_relay_url
                    .as_ref()
                    .and_then(|rru| RelayUrl::try_from_unchecked_url(rru).ok())
                    .into_vec(),
                marker: None,
            }),
            ParsedTag::RootAddress { address, marker: _ } => Some(EventReference::Addr(address)),
            ParsedTag::RootEvent {
                id,
                recommended_relay_url,
                marker,
                author_pubkey,
            } => Some(EventReference::Id {
                id,
                author: author_pubkey,
                relays: recommended_relay_url
                    .as_ref()
                    .and_then(|rru| RelayUrl::try_from_unchecked_url(rru).ok())
                    .into_vec(),
                marker,
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parsed_tag() {
        let inputs = [
            vec!["a", "30023:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:abcd", "wss://nostr.example.com"],
            vec!["content-warning", "nsfw"],
            vec!["delegation", "f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca", "kind=1&created_at<1675721813", "6f44d7fe4f1c09f3954640fb58bd12bae8bb8ff4120853c4693106c82e920e2b898f1f9ba9bd65449a987c39c0423426ab7b53910c0c6abfb41b30bc16e5f524"],
            vec!["e", "5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36", "wss://nostr.example.com", "f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca"],
            vec!["e", "5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36", "", "f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca"],
            vec!["t", "bitcoin"],
            vec!["d", "20241214-blog"],
            vec!["k", "1111"],
            vec!["nonce", "24234234", "24"],
            vec!["proxy", "blah blah", "mastodon bridge"],
            vec!["p", "f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca"],
            vec!["q", "5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36", "wss://nos.lol"],
            vec!["q", "5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36", "", "f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca"],
            vec!["r", "wss://chorus.mikedilger.com:444", "rw"],
            vec!["A", "30023:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:abcd", "wss://nostr.example.com"],
            vec!["E", "5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36", "wss://nostr.example.com", "f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca"],
            vec!["K", "1"],
            vec!["P", "f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca"],
            vec!["subject", "crazy stuff"],
            vec!["summary", "dont bother"],
            vec!["title", "my blog"],
        ];

        for input in inputs {
            println!("{:?}", input);

            let v = input.iter().map(|s| s.to_string()).collect();
            let tag = Tag::from_strings(v);

            // Be sure it parses without error
            let parsed_tag = tag.parse().unwrap();

            // Be sure it did not parse as an Unmatched tag
            assert!(!matches!(parsed_tag, ParsedTag::Unmatched(ref _v)));

            // Be sure it converts back identically
            assert_eq!(tag, parsed_tag.into_tag());
        }
    }
}
