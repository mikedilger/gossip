use crate::{globals::GLOBALS, people::DbPerson};
use nostr_types::{
    ContentSegment, Event, EventDelegation, EventKind, Id, MilliSatoshi, NostrBech32, PublicKeyHex,
    ShatteredContent, Tag,
};

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
    /// The total amount of MilliSatoshi zapped to this note
    pub(super) zaptotal: MilliSatoshi,
    /// Has the current user reacted to this post?
    pub(super) self_already_reacted: bool,
    /// The content shattered into renderable elements
    pub(super) shattered_content: ShatteredContent,
}

impl NoteData {
    pub(super) fn new(event: Event) -> NoteData {
        // We do not filter event kinds here anymore. The feed already does that.
        // There is no sense in duplicating that work.

        let delegation = event.delegation();

        let deletion = GLOBALS.storage.get_deletion(event.id).unwrap_or(None);

        let (reactions, self_already_reacted) = GLOBALS
            .storage
            .get_reactions(event.id)
            .unwrap_or((vec![], false));

        let zaptotal = GLOBALS
            .storage
            .get_zap_total(event.id)
            .unwrap_or(MilliSatoshi(0));

        // build a list of all cached mentions and their index
        // only notes that are in the cache will be rendered as reposts
        let mentions = {
            let mut mentions = Vec::<(usize, Id)>::new();
            for (i, tag) in event.tags.iter().enumerate() {
                if let Tag::Event { id, .. } = tag {
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

        NoteData {
            event,
            delegation,
            author,
            deletion,
            repost,
            embedded_event,
            mentions,
            reactions,
            zaptotal,
            self_already_reacted,
            shattered_content,
        }
    }

    pub(super) fn update_reactions(&mut self) {
        let (mut reactions, self_already_reacted) = GLOBALS
            .storage
            .get_reactions(self.event.id)
            .unwrap_or((vec![], false));

        self.reactions.clear();
        self.reactions.append(&mut reactions);

        self.self_already_reacted = self_already_reacted;
    }
}
