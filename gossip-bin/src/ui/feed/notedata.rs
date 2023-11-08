use gossip_lib::GLOBALS;
use gossip_lib::{Person, PersonList};
use std::collections::HashMap;

use nostr_types::{
    ContentSegment, Event, EventDelegation, EventKind, Id, MilliSatoshi, NostrBech32, PublicKey,
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
    /// Kind 16 generic repost, has 'k' and 'e' tag, and the reposted note's JSON
    /// is optionally included in the content
    GenericRepost,
}

pub(super) struct NoteData {
    /// Original Event object, as received from nostr
    pub(super) event: Event,
    /// Delegation status of this event
    pub(super) delegation: EventDelegation,
    /// Author of this note (considers delegation)
    pub(super) author: Person,
    /// Lists the author is on
    pub(super) lists: HashMap<PersonList, bool>,
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
    /// error content (gossip-created notations)
    pub(super) error_content: Option<String>,
    /// Securely delivered via GiftWrap
    pub(super) secure: bool,
}

impl NoteData {
    pub(super) fn new(mut event: Event) -> NoteData {
        // We do not filter event kinds here anymore. The feed already does that.
        // There is no sense in duplicating that work.

        let mut secure: bool = false;
        if matches!(event.kind, EventKind::GiftWrap) {
            secure = true;
            if let Ok(rumor) = GLOBALS.signer.unwrap_giftwrap(&event) {
                // Use the rumor for subsequent processing
                let id = event.id;
                event = rumor.into_event_with_bad_signature();
                event.id = id; // lie, keep the giftwrap id
            }
        }

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
            if event.kind == EventKind::Repost || event.kind == EventKind::GenericRepost {
                if !event.content.trim().is_empty() {
                    if let Ok(embedded_event) = serde_json::from_str::<Event>(&event.content) {
                        if event.kind == EventKind::Repost
                            || (event.kind == EventKind::GenericRepost
                                && embedded_event.kind.is_feed_displayable())
                        {
                            Some(embedded_event)
                        } else {
                            None
                        }
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
        let (display_content, error_content) = match event.kind {
            EventKind::TextNote => (event.content.trim().to_string(), None),
            EventKind::Repost => ("".to_owned(), None),
            EventKind::GenericRepost => ("".to_owned(), None),
            EventKind::EncryptedDirectMessage => match GLOBALS.signer.decrypt_message(&event) {
                Ok(m) => (m, None),
                Err(_) => ("".to_owned(), Some("DECRYPTION FAILED".to_owned())),
            },
            EventKind::LongFormContent => (event.content.clone(), None),
            EventKind::DmChat => (event.content.clone(), None),
            EventKind::GiftWrap => ("".to_owned(), Some("DECRYPTION FAILED".to_owned())),
            EventKind::ChannelMessage => (event.content.clone(), None),
            EventKind::LiveChatMessage => (event.content.clone(), None),
            EventKind::CommunityPost => (event.content.clone(), None),
            EventKind::DraftLongFormContent => (event.content.clone(), None),
            k => {
                let kind_number: u32 = k.into();
                let mut dc = format!("UNSUPPORTED EVENT KIND {}", kind_number);
                // support the 'alt' tag of NIP-31:
                for tag in &event.tags {
                    if let Tag::Other { tag, data } = tag {
                        if tag == "alt" && !data.is_empty() {
                            dc =
                                format!("UNSUPPORTED EVENT KIND {}, ALT: {}", kind_number, data[0]);
                        }
                    }
                }
                ("".to_owned(), Some(dc))
            }
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
            } else if event.kind == EventKind::GenericRepost {
                Some(RepostType::GenericRepost)
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
        let author_pubkey: PublicKey = if let EventDelegation::DelegatedBy(pubkey) = delegation {
            pubkey
        } else {
            event.pubkey
        };

        let author = match GLOBALS.storage.read_person(&author_pubkey) {
            Ok(Some(p)) => p,
            _ => Person::new(author_pubkey),
        };

        let lists = match GLOBALS.storage.read_person_lists(&author_pubkey) {
            Ok(lists) => lists,
            _ => HashMap::new(),
        };

        NoteData {
            event,
            delegation,
            author,
            lists,
            deletion,
            repost,
            embedded_event,
            mentions,
            reactions,
            zaptotal,
            self_already_reacted,
            shattered_content,
            error_content,
            secure,
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

    #[allow(dead_code)]
    pub(super) fn followed(&self) -> bool {
        self.lists.contains_key(&PersonList::Followed)
    }

    pub(super) fn muted(&self) -> bool {
        self.lists.contains_key(&PersonList::Muted)
    }
}
