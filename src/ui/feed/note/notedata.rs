use crate::{
    globals::{Globals, GLOBALS},
    people::DbPerson,
};
use nostr_types::{
    ContentSegment, Event, EventDelegation, EventKind, Id, NostrBech32, PublicKeyHex,
    ShatteredContent, Tag,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[derive(PartialEq)]
pub(super) enum RepostType {
    /// Damus style, kind 6 repost where the reposted note's JSON
    /// is included in the content
    Kind6Embedded,
    /// kind 6 repost without reposted note, but has a mention tag
    Kind6Mention,
    /// Post only has whitespace and a single mention tag
    MentionOnly,
    /// Post has a comment and at least one mention tag
    CommentMention,
}

pub(super) struct NoteData {
    /// Original Event object, as received from nostr
    pub(super) event: Event,
    /// Delegation status of this event
    pub(super) delegation: EventDelegation,
    /// Author of this note (considers delegation)
    pub(super) author: DbPerson,
    /// Deletion reason if any
    pub(super) deletion: Option<String>,
    /// Do we consider this note as being a repost of another?
    pub(super) repost: Option<RepostType>,
    /// Optional embedded event of kind:6 repost
    pub(super) embedded_event: Option<Event>,
    /// A list of mentioned events and their index: (index, event)
    pub(super) mentions: Vec<(usize, Id)>,
    /// Known reactions to this post
    pub(super) reactions: Vec<(char, usize)>,
    /// Has the current user reacted to this post?
    pub(super) self_already_reacted: bool,
    /// The content shattered into renderable elements
    pub(super) shattered_content: ShatteredContent,
}

impl NoteData {
    pub(super) fn new(event: Event) -> Option<NoteData> {
        // We do not filter event kinds here anymore. The feed already does that.
        // There is no sense in duplicating that work.

        let delegation = event.delegation();

        let deletion = Globals::get_deletion_sync(event.id);

        let (reactions, self_already_reacted) = Globals::get_reactions_sync(event.id);

        // build a list of all cached mentions and their index
        // only notes that are in the cache will be rendered as reposts
        let mentions = {
            let mut mentions = Vec::<(usize, Id)>::new();
            for (i, tag) in event.tags.iter().enumerate() {
                if let Tag::Event {
                    id,
                    recommended_relay_url: _,
                    marker: _,
                } = tag
                {
                    mentions.push((i, *id));
                }
            }
            mentions
        };

        let embedded_event = {
            if event.kind == EventKind::Repost {
                if !event.content.trim().is_empty() {
                    if let Ok(event) = serde_json::from_str::<Event>(&event.content) {
                        Some(event)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        // Compute the content to our needs
        let display_content = match event.kind {
            EventKind::TextNote => event.content.trim().to_string(),
            EventKind::Repost => {
                if !event.content.trim().is_empty() && embedded_event.is_none() {
                    "REPOSTED EVENT IS NOT RELEVANT".to_owned()
                } else {
                    "".to_owned()
                }
            }
            EventKind::EncryptedDirectMessage => match GLOBALS.signer.decrypt_message(&event) {
                Ok(m) => m,
                Err(_) => "DECRYPTION FAILED".to_owned(),
            },
            EventKind::LongFormContent => event.content.clone(),
            _ => "NON FEED RELATED EVENT".to_owned(),
        };

        // shatter content here so we can use it in our content analysis
        let mut shattered_content = ShatteredContent::new(display_content);

        let mut has_tag_reference = false;
        let mut has_nostr_event_reference = false;
        for shard in &shattered_content.segments {
            match shard {
                ContentSegment::NostrUrl(nurl) => match nurl.0 {
                    NostrBech32::Id(_) | NostrBech32::EventPointer(_) => {
                        has_nostr_event_reference = true;
                    }
                    _ => (),
                },
                ContentSegment::TagReference(_) => {
                    has_tag_reference = true;
                }
                ContentSegment::Hyperlink(_) => (),
                ContentSegment::Plain(_) => (),
            }
        }

        let repost = {
            let content_trim = event.content.trim();

            if event.kind == EventKind::Repost && embedded_event.is_some() {
                Some(RepostType::Kind6Embedded)
            } else if has_tag_reference || has_nostr_event_reference || content_trim.is_empty() {
                if !mentions.is_empty() {
                    if content_trim.is_empty() {
                        // handle NIP-18 conform kind:6 with 'e' tag but no content
                        shattered_content
                            .segments
                            .push(ContentSegment::TagReference(0));

                        if event.kind == EventKind::Repost {
                            Some(RepostType::Kind6Mention)
                        } else {
                            Some(RepostType::MentionOnly)
                        }
                    } else {
                        Some(RepostType::CommentMention)
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        // If delegated, use the delegated person
        let author_pubkey: PublicKeyHex = if let EventDelegation::DelegatedBy(pubkey) = delegation {
            pubkey.into()
        } else {
            event.pubkey.into()
        };

        let author = match GLOBALS.people.get(&author_pubkey) {
            Some(p) => p,
            None => DbPerson::new(author_pubkey),
        };

        Some(NoteData {
            event,
            delegation,
            author,
            deletion,
            repost,
            embedded_event,
            mentions,
            reactions,
            self_already_reacted,
            shattered_content,
        })
    }

    pub(super) fn update_reactions(&mut self) {
        let (mut reactions, self_already_reacted) = Globals::get_reactions_sync(self.event.id);

        self.reactions.clear();
        self.reactions.append(&mut reactions);

        self.self_already_reacted = self_already_reacted;
    }
}

/// a 'note' is a processed event
pub struct Notes {
    notes: HashMap<Id, Rc<RefCell<NoteData>>>,
}

impl Notes {
    pub fn new() -> Notes {
        Notes {
            notes: HashMap::new(),
        }
    }

    /*
    /// Drop NoteData objects that do not have a
    /// correlated event in the event cache
    pub(super) fn cache_invalidate_missing_events(&mut self) {
        self.notes.retain(|id,_| GLOBALS.events.contains_key(id));
    }
     */

    /*
    /// Drop NoteData for a specific note
    pub(super) fn cache_invalidate_note(&mut self, id: &Id) {
        self.notes.remove(id);
    }
     */

    /// Drop all NoteData for a given person
    pub(in crate::ui) fn cache_invalidate_person(&mut self, pubkey: &PublicKeyHex) {
        self.notes
            .retain(|_, note| note.borrow().author.pubkey != *pubkey);
    }

    pub(super) fn try_update_and_get(&mut self, id: &Id) -> Option<Rc<RefCell<NoteData>>> {
        if self.notes.contains_key(id) {
            // get a mutable reference to update reactions, then give it back
            if let Some(pair) = self.notes.get(id) {
                if let Ok(mut mut_ref) = pair.try_borrow_mut() {
                    mut_ref.update_reactions();
                }
            }
            // return from cache
            return self._try_get_and_borrow(id);
        } else {
            // otherwise try to create new and add to cache
            if let Some(event) = GLOBALS.events.get(id) {
                if let Some(note) = NoteData::new(event) {
                    // add to cache
                    let ref_note = Rc::new(RefCell::new(note));
                    self.notes.insert(*id, ref_note);
                    return self._try_get_and_borrow(id);
                }
            } else {
                // send a worker to try and load it from the database
                // if it's in the db it will go into the cache and be
                // available on the next UI update
                let id_copy = id.to_owned();
                tokio::spawn(async move {
                    if let Err(e) = GLOBALS.events.get_local(id_copy).await {
                        tracing::error!("{}", e);
                    }
                });
            }
        }

        None
    }

    /*
    pub(super) fn try_get(&mut self, id: &Id) -> Option<Rc<RefCell<NoteData>>> {
        if self.notes.contains_key(id) {
            // return from cache
            return self._try_get_and_borrow(id)
        } else {
            // otherwise try to create new and add to cache
            if let Some(event) = GLOBALS.events.get(id) {
                if let Some(note) = NoteData::new(event) {
                    // add to cache
                    let ref_note = Rc::new(RefCell::new(note));
                    self.notes.insert(*id, ref_note);
                    return self._try_get_and_borrow(id);
                }
            } else {
                // send a worker to try and load it from the database
                // if it's in the db it will go into the cache and be
                // available on the next UI update
                let id_copy = id.to_owned();
                tokio::spawn(async move {
                    if let Err(e) = GLOBALS.events.get_local(id_copy).await {
                        tracing::error!("{}", e);
                    }
                });
            }
        }
        None
    }
     */

    fn _try_get_and_borrow(&self, id: &Id) -> Option<Rc<RefCell<NoteData>>> {
        if let Some(value) = self.notes.get(id) {
            return Some(value.clone());
        }
        None
    }
}
