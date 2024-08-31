use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use nostr_types::{
    ContentEncryptionAlgorithm, Event, EventKind, EventReference, Id, PreEvent, RelayUrl, Tag,
    Unixtime,
};

pub struct BookmarkList(Vec<(EventReference, bool)>);

impl BookmarkList {
    pub fn empty() -> Self {
        Self(Vec::new())
    }

    pub fn clear(&mut self) {
        self.0 = Vec::new();
    }

    fn add_tags(&mut self, tags: &[Tag], private: bool) -> Result<(), Error> {
        for tag in tags.iter() {
            let bookmark = match tag.tagname() {
                "e" => {
                    let (id, opturl, optmarker) = tag.parse_event()?;
                    let relays = match opturl {
                        Some(url) => match RelayUrl::try_from_unchecked_url(&url) {
                            Ok(rurl) => vec![rurl],
                            Err(_) => vec![],
                        },
                        None => vec![],
                    };
                    EventReference::Id {
                        id,
                        author: None,
                        relays,
                        marker: optmarker,
                    }
                }
                "a" => {
                    let (addr, _optmarker) = tag.parse_address()?;
                    EventReference::Addr(addr)
                }
                // We don't support other tags (but we have to preserve them)
                _ => continue,
            };

            self.0.push((bookmark, private));
        }

        Ok(())
    }

    pub fn add(&mut self, er: EventReference, private: bool) -> Result<bool, Error> {
        let index = self.0.iter().position(|(thiser, _)| *thiser == er);
        if index.is_some() {
            return Ok(false);
        }
        self.0.push((er, private));
        Ok(true)
    }

    pub fn remove(&mut self, er: EventReference) -> Result<bool, Error> {
        let index = self.0.iter().position(|(thiser, _)| *thiser == er);
        match index {
            None => Ok(false),
            Some(index) => {
                self.0.remove(index);
                Ok(true)
            }
        }
    }

    pub async fn from_event(event: &Event) -> Result<Self, Error> {
        let public_key = match GLOBALS.identity.public_key() {
            None => return Err(ErrorKind::NoPublicKey.into()),
            Some(pk) => pk,
        };

        if event.kind != EventKind::BookmarkList {
            return Err(ErrorKind::WrongEventKind.into());
        }

        if event.pubkey != public_key {
            return Err(ErrorKind::General("Event by wrong author".to_string()).into());
        }

        let mut bml = Self::empty();
        bml.add_tags(event.tags.as_ref(), false)?;
        if let Ok(json_string) = GLOBALS.identity.decrypt(&public_key, &event.content).await {
            if let Ok(vectags) = serde_json::from_str::<Vec<Tag>>(&json_string) {
                bml.add_tags(vectags.as_ref(), true)?;
            }
        }

        Ok(bml)
    }

    pub async fn into_event(&self) -> Result<Event, Error> {
        let public_key = match GLOBALS.identity.public_key() {
            None => return Err(ErrorKind::NoPublicKey.into()),
            Some(pk) => pk,
        };

        let er_to_tag = |er: &EventReference| -> Tag {
            match er {
                EventReference::Id { id, relays, .. } => {
                    Tag::new_event(*id, relays.first().map(|r| r.to_unchecked_url()), None)
                }
                EventReference::Addr(ea) => Tag::new_address(ea, None),
            }
        };

        let tags: Vec<Tag> = self
            .0
            .iter()
            .filter_map(
                |(er, private)| {
                    if *private {
                        None
                    } else {
                        Some(er_to_tag(er))
                    }
                },
            )
            .collect();

        let content = {
            let private: Vec<Tag> = self
                .0
                .iter()
                .filter_map(
                    |(er, private)| {
                        if *private {
                            Some(er_to_tag(er))
                        } else {
                            None
                        }
                    },
                )
                .collect();
            let private_json = serde_json::to_string(&private)?;
            GLOBALS
                .identity
                .encrypt(
                    &public_key,
                    &private_json,
                    ContentEncryptionAlgorithm::Nip04,
                )
                .await?
        };

        let pre_event = PreEvent {
            pubkey: public_key,
            created_at: Unixtime::now(),
            kind: EventKind::BookmarkList,
            tags,
            content,
        };

        GLOBALS.identity.sign_event(pre_event).await
    }

    pub async fn get_bookmark_feed(&self) -> Result<Vec<Id>, Error> {
        let mut feed: Vec<Id> = Vec::new();
        for (eref, _) in &self.0 {
            match eref {
                EventReference::Id { id, .. } => feed.push(*id),
                EventReference::Addr(ea) => {
                    if let Some(event) = GLOBALS
                        .db()
                        .get_replaceable_event(ea.kind, ea.author, &ea.d)
                        .await?
                    {
                        feed.push(event.id);
                    }
                }
            }
        }
        Ok(feed)
    }
}
