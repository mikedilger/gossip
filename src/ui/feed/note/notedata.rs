use dashmap::DashMap;
use nostr_types::{Event, EventDelegation, EventKind, Id, NostrBech32, PublicKeyHex, Tag};
use crate::{people::DbPerson, globals::{Globals, GLOBALS}};
use super::shatter::{ShatteredContent, shatter_content, ContentSegment};

#[derive(PartialEq)]
pub(crate) enum RepostType {
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

pub(crate) struct NoteData {
    /// Original Event object, as received from nostr
    pub(crate) event: Event,
    /// Delegation status of this event
    pub(crate) delegation: EventDelegation,
    /// Author of this note (considers delegation)
    pub(crate) author: DbPerson,
    /// Deletion reason if any
    pub(crate) deletion: Option<String>,
    /// Do we consider this note as being a repost of another?
    pub(crate) repost: Option<RepostType>,
    /// A list of CACHED mentioned events and their index: (index, event)
    pub(crate) cached_mentions: Vec<(usize, Event)>,
    /// Known reactions to this post
    pub(crate) reactions: Vec<(char, usize)>,
    /// Has the current user reacted to this post?
    pub(crate) self_already_reacted: bool,
    /// The content shattered into renderable elements
    pub(crate) shattered_content: ShatteredContent,
}

impl NoteData {
    pub fn new(event: Event, with_inline_mentions: bool, show_long_form: bool) -> Option<NoteData> {
        // We do not filter event kinds here anymore. The feed already does that.
        // There is no sense in duplicating that work.

        let delegation = event.delegation();

        let deletion = Globals::get_deletion_sync(event.id);

        let (reactions, self_already_reacted) = Globals::get_reactions_sync(event.id);

        // build a list of all cached mentions and their index
        // only notes that are in the cache will be rendered as reposts
        let cached_mentions = {
            let mut cached_mentions = Vec::<(usize, Event)>::new();
            for (i, tag) in event.tags.iter().enumerate() {
                if let Tag::Event {
                    id,
                    recommended_relay_url: _,
                    marker: _,
                } = tag
                {
                    // grab all cached 'e' tags, we will decide whether
                    // to use them after parsing content
                    if let Some(event) = GLOBALS.events.get(id) {
                        cached_mentions.push((i, event));
                    }
                }
            }
            cached_mentions
        };

        // Compute the content to our needs
        let display_content = match event.kind {
            EventKind::TextNote | EventKind::Repost => event.content.trim().to_string(),
            EventKind::EncryptedDirectMessage => match GLOBALS.signer.decrypt_message(&event) {
                Ok(m) => m,
                Err(_) => "DECRYPTION FAILED".to_owned(),
            },
            EventKind::LongFormContent => event.content.clone(),
            _ => "NON FEED RELATED EVENT".to_owned(),
        };

        // shatter content here so we can use it in our content analysis
        let mut shattered_content = shatter_content(display_content);

        let mut has_tag_reference = false;
        let mut has_nostr_event_reference = false;
        for shard in &shattered_content.segments {
            match shard {
                ContentSegment::NostrUrl(nurl) => match nurl.0 {
                    NostrBech32::Id(_) | NostrBech32::EventPointer(_) => {
                        has_nostr_event_reference = true;
                    },
                    _ => (),
                },
                ContentSegment::TagReference(_) => {
                    has_tag_reference = true;
                },
                ContentSegment::Hyperlink(_) => (),
                ContentSegment::Plain(_) => (),
            }
        }

        let repost = {
            let content_trim = event.content.trim();
            let content_trim_len = content_trim.chars().count();
            if event.kind == EventKind::Repost
                && serde_json::from_str::<Event>(&event.content).is_ok()
            {
                if !show_long_form {
                    let inner = serde_json::from_str::<Event>(&event.content).unwrap();
                    if inner.kind == EventKind::LongFormContent {
                        return None;
                    }
                }
                Some(RepostType::Kind6Embedded)
            } else if has_tag_reference || has_nostr_event_reference || content_trim.is_empty() {
                if !cached_mentions.is_empty() {
                    if content_trim.is_empty() {
                        // handle NIP-18 conform kind:6 with 'e' tag but no content
                        shattered_content
                            .segments
                            .push(ContentSegment::TagReference(0));
                    }
                    if event.kind == EventKind::Repost {
                        Some(RepostType::Kind6Mention)
                    } else {
                        Some(RepostType::MentionOnly)
                    }
                } else {
                    None
                }
            } else if with_inline_mentions
                && content_trim_len > 4
                && content_trim.chars().nth(content_trim_len - 1).unwrap() == ']'
                && content_trim.chars().nth(content_trim_len - 3).unwrap() == '['
                && content_trim.chars().nth(content_trim_len - 4).unwrap() == '#'
                && !cached_mentions.is_empty()
            {
                // matches content that ends with a mention, avoiding use of a regex match
                Some(RepostType::CommentMention)
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
            cached_mentions,
            reactions,
            self_already_reacted,
            shattered_content,
        })
    }
}

/// a 'note' is a processed event
pub struct Notes {
    notes: DashMap<Id, NoteData>,
}

impl Notes {
    pub fn new() -> Notes {
        Notes {
            notes: DashMap::new(),
        }
    }
}
