use gossip_lib::GLOBALS;
use gossip_lib::{Person, PersonList, PersonTable, Private, Table};
use std::collections::HashMap;

use nostr_types::{
    ContentSegment, Event, EventDelegation, EventKind, EventReference, Id, MilliSatoshi, NAddr,
    NostrBech32, PublicKey, RelayUrl, ShatteredContent, Unixtime,
};

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
    /// Kind 16 generic repost, has 'k' and 'e' tag, and the reposted note's JSON
    /// is optionally included in the content
    GenericRepost,
}

#[derive(PartialEq, Default)]
pub(crate) enum EncryptionType {
    #[default]
    None,
    Nip04,
    Giftwrap,
}

pub(crate) struct NoteData {
    /// Original Event object, as received from nostr
    pub event: Event,

    /// Delegation status of this event
    pub delegation: EventDelegation,

    /// Author of this note (considers delegation)
    pub author: Person,

    /// Lists the author is on
    pub lists: HashMap<PersonList, Private>,

    /// Deletion reasons if any
    pub deletions: Vec<String>,

    /// Annotations by the author
    pub annotations: Vec<(Unixtime, String)>,

    /// Do we consider this note as being a repost of another?
    pub repost: Option<RepostType>,

    /// Optional embedded event of kind:6 repost
    pub embedded_event: Option<Event>,

    /// A list of mentioned events and their index: (index, event)
    pub mentions: Vec<(usize, Id)>,

    /// Known reactions to this post
    pub reactions: Vec<(char, usize)>,

    /// Has the current user reacted to this post?
    pub self_already_reacted: bool,

    /// The total amount of MilliSatoshi zapped to this note
    pub zaptotal: MilliSatoshi,

    /// Relays this event was seen on and when, if any
    pub seen_on: Vec<(RelayUrl, Unixtime)>,

    /// The content shattered into renderable elements
    pub shattered_content: ShatteredContent,

    /// error content (gossip-created notations)
    pub error_content: Option<String>,

    /// direct message
    pub direct_message: bool,

    /// Encryption type this note had on the network
    pub encryption: EncryptionType,

    /// Bookmarked
    pub bookmarked: bool,
}

impl NoteData {
    pub fn new(mut event: Event) -> NoteData {
        // We do not filter event kinds here anymore. The feed already does that.
        // There is no sense in duplicating that work.

        let mut encryption = EncryptionType::None;
        let mut direct_message: bool = false;
        if matches!(event.kind, EventKind::GiftWrap) {
            direct_message = true;
            encryption = EncryptionType::Giftwrap;
            // Use the rumor for subsequent processing, but swap for the Giftwrap's id
            // since that is the effective event (database-accessible, deletable, etc)
            if let Ok(rumor) = GLOBALS.identity.unwrap_giftwrap(&event) {
                let id = event.id;
                event = rumor.into_event_with_bad_signature();
                event.id = id; // lie, keep the giftwrap id
            }
        } else if matches!(event.kind, EventKind::EncryptedDirectMessage) {
            direct_message = true;
            encryption = EncryptionType::Nip04;
        }

        let delegation = event.delegation();

        // This function checks that the deletion author is allowed
        let deletions = GLOBALS.storage.get_deletions(&event).unwrap_or_default();

        // This function checks the authors match
        let annotations = GLOBALS.storage.get_annotations(&event).unwrap_or_default();

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
                if let Ok((id, _, _)) = tag.parse_event() {
                    mentions.push((i, id));
                }
            }
            mentions
        };

        let (embedded_event, embedded_event_error) = {
            if event.kind == EventKind::Repost || event.kind == EventKind::GenericRepost {
                if !event.content.trim().is_empty() {
                    let result = serde_json::from_str::<Event>(&event.content);
                    if let Ok(embedded_event) = result {
                        if embedded_event.kind.is_feed_displayable() {
                            (Some(embedded_event), None)
                        } else {
                            (None, {
                                let kind_number: u32 = embedded_event.kind.into();
                                Some(format!(
                                    "ERROR EMBEDDED EVENT UNSUPPORTED KIND : {}",
                                    kind_number
                                ))
                            })
                        }
                    } else {
                        (
                            None,
                            Some(format!(
                                "ERROR PARSING EMBEDDED EVENT: '{}'",
                                result.err().unwrap()
                            )),
                        )
                    }
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        };

        // Compute the content to our needs
        let (display_content, error_content) = match event.kind {
            EventKind::TextNote => (event.content.trim().to_string(), None),
            EventKind::Repost => ("".to_owned(), embedded_event_error),
            EventKind::GenericRepost => ("".to_owned(), None),
            EventKind::EncryptedDirectMessage => {
                match GLOBALS.identity.decrypt_event_contents(&event) {
                    Ok(m) => (m, None),
                    Err(_) => ("".to_owned(), Some("DECRYPTION FAILED".to_owned())),
                }
            }
            EventKind::LongFormContent => (event.content.clone(), None),
            EventKind::DmChat => (event.content.clone(), None),
            EventKind::GiftWrap => ("".to_owned(), Some("DECRYPTION FAILED".to_owned())),
            EventKind::ChannelMessage => (event.content.clone(), None),
            EventKind::LiveChatMessage => (event.content.clone(), None),
            EventKind::CommunityPost => (event.content.clone(), None),
            EventKind::DraftLongFormContent => (event.content.clone(), None),
            k => {
                if k.is_feed_displayable() {
                    (event.content.clone(), Some(format!("{:?}", k)))
                } else {
                    let mut dc = format!("UNSUPPORTED EVENT KIND {:?}", k);
                    // support the 'alt' tag of NIP-31:
                    for tag in &event.tags {
                        if tag.tagname() == "alt" && tag.value() != "" {
                            dc = format!("UNSUPPORTED EVENT KIND {:?}, ALT: {}", k, tag.value());
                        }
                    }
                    ("".to_owned(), Some(dc))
                }
            }
        };

        // shatter content here so we can use it in our content analysis
        let mut shattered_content = ShatteredContent::new(display_content);

        let mut has_tag_reference = false;
        let mut has_nostr_event_reference = false;
        for shard in &shattered_content.segments {
            match shard {
                ContentSegment::NostrUrl(nurl) => match nurl.0 {
                    NostrBech32::Id(_) | NostrBech32::NEvent(_) => {
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
                    if let Some((tag, _)) = mentions.first() {
                        shattered_content
                            .segments
                            .push(ContentSegment::TagReference(*tag));
                    }
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

        let author = match PersonTable::read_record(author_pubkey, None) {
            Ok(Some(p)) => p,
            _ => Person::new(author_pubkey),
        };

        let lists = match GLOBALS.storage.read_person_lists(&author_pubkey) {
            Ok(lists) => lists,
            _ => HashMap::new(),
        };

        let seen_on = GLOBALS
            .storage
            .get_event_seen_on_relay(event.id)
            .unwrap_or_default();

        let bookmarked = GLOBALS.current_bookmarks.read().contains(&event.id);

        NoteData {
            event,
            delegation,
            author,
            lists,
            deletions,
            annotations,
            repost,
            embedded_event,
            mentions,
            reactions,
            self_already_reacted,
            zaptotal,
            seen_on,
            shattered_content,
            error_content,
            direct_message,
            encryption,
            bookmarked,
        }
    }

    pub(super) fn update(&mut self) {
        // Update reactions
        let (mut reactions, self_already_reacted) = GLOBALS
            .storage
            .get_reactions(self.event.id)
            .unwrap_or((vec![], false));
        self.reactions.clear();
        self.reactions.append(&mut reactions);
        self.self_already_reacted = self_already_reacted;

        // Update seen_on
        let mut seen_on = GLOBALS
            .storage
            .get_event_seen_on_relay(self.event.id)
            .unwrap_or_default();

        self.seen_on.clear();
        self.seen_on.append(&mut seen_on);

        // Update annotations
        self.annotations = GLOBALS
            .storage
            .get_annotations(&self.event)
            .unwrap_or_default();

        // Update zaptotal
        self.zaptotal = GLOBALS
            .storage
            .get_zap_total(self.event.id)
            .unwrap_or(MilliSatoshi(0));
    }

    #[allow(dead_code)]
    pub(super) fn followed(&self) -> bool {
        self.lists.contains_key(&PersonList::Followed)
    }

    pub(super) fn muted(&self) -> bool {
        self.lists.contains_key(&PersonList::Muted)
    }

    pub(super) fn event_reference(&self) -> EventReference {
        if self.event.kind.is_replaceable() {
            EventReference::Addr(NAddr {
                d: self.event.parameter().unwrap_or("".to_owned()),
                relays: vec![],
                kind: self.event.kind,
                author: self.event.pubkey,
            })
        } else {
            EventReference::Id {
                id: self.event.id,
                author: None,
                relays: vec![],
                marker: None,
            }
        }
    }
}
