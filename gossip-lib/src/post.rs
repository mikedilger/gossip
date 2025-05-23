use crate::dm_channel::DmChannel;
use crate::error::{Error, ErrorKind};
use crate::fetcher::FetchResult;
use crate::globals::GLOBALS;
use crate::relay;
use crate::relay::Relay;
use nostr_types::{
    ContentEncryptionAlgorithm, ContentSegment, Event, EventKind, EventReference, FileMetadata, Id,
    NAddr, NostrBech32, ParsedTag, PreEvent, PublicKey, RelayUrl, ShatteredContent, Tag,
    UncheckedUrl, Unixtime, Url,
};
use std::sync::mpsc;

pub async fn prepare_post_normal(
    author: PublicKey,
    content: String,
    mut tags: Vec<Tag>,
    in_reply_to: Option<Event>,
    annotation: bool,
) -> Result<Vec<(Event, Vec<RelayUrl>)>, Error> {
    add_gossip_tag(&mut tags);

    if annotation {
        tags.push(Tag::new(&["annotation"]))
    }

    if let Some(ref parent) = in_reply_to {
        add_thread_based_tags(author, &mut tags, parent)?;
    }

    add_tags_mirroring_content(&content, &mut tags, false).await;

    let pre_event = PreEvent {
        pubkey: author,
        created_at: Unixtime::now(),
        kind: EventKind::TextNote,
        tags,
        content,
    };

    let event = {
        let powint = GLOBALS.db().read_setting_pow();
        if powint > 0 {
            let (work_sender, work_receiver) = mpsc::channel();
            std::thread::spawn(move || {
                work_logger(work_receiver, powint);
            });
            GLOBALS
                .identity
                .sign_event_with_pow(pre_event, powint, Some(work_sender))
                .await?
        } else {
            GLOBALS.identity.sign_event(pre_event).await?
        }
    };

    let relays = relay::relays_to_post_to(&event)?;

    Ok(vec![(event, relays)])
}

pub async fn prepare_post_comment(
    author: PublicKey,
    content: String,
    mut tags: Vec<Tag>,
    parent: Event,
    annotation: bool,
) -> Result<Vec<(Event, Vec<RelayUrl>)>, Error> {
    add_gossip_tag(&mut tags);

    if annotation {
        tags.push(Tag::new(&["annotation"]))
    }

    if copy_root_tags(&mut tags, &parent) < 1 {
        set_parent_as_root_tags(&mut tags, &parent);
    }

    add_parent_tags(&mut tags, &parent, author);

    add_tags_mirroring_content(&content, &mut tags, false).await;

    let pre_event = PreEvent {
        pubkey: author,
        created_at: Unixtime::now(),
        kind: EventKind::Comment,
        tags,
        content,
    };

    let event = {
        let powint = GLOBALS.db().read_setting_pow();
        if powint > 0 {
            let (work_sender, work_receiver) = mpsc::channel();
            std::thread::spawn(move || {
                work_logger(work_receiver, powint);
            });
            GLOBALS
                .identity
                .sign_event_with_pow(pre_event, powint, Some(work_sender))
                .await?
        } else {
            GLOBALS.identity.sign_event(pre_event).await?
        }
    };

    let relays = relay::relays_to_post_to(&event)?;

    Ok(vec![(event, relays)])
}

pub async fn prepare_post_nip04(
    author: PublicKey,
    content: String,
    dm_channel: DmChannel,
    annotation: bool,
) -> Result<Vec<(Event, Vec<RelayUrl>)>, Error> {
    if dm_channel.keys().len() > 1 {
        return Err(ErrorKind::GroupDmsNotSupported.into());
    }

    let recipient = if dm_channel.keys().is_empty() {
        author // must be to yourself
    } else {
        dm_channel.keys()[0]
    };

    let content = GLOBALS
        .identity
        .encrypt(&recipient, &content, ContentEncryptionAlgorithm::Nip04)
        .await?;

    let mut tags = vec![ParsedTag::Pubkey {
        pubkey: recipient,
        recommended_relay_url: None, // FIXME,
        petname: None,
    }
    .into_tag()];
    if annotation {
        tags.push(Tag::new(&["annotation"]))
    }

    let pre_event = PreEvent {
        pubkey: author,
        created_at: Unixtime::now(),
        kind: EventKind::EncryptedDirectMessage,
        tags,
        content,
    };

    let event = GLOBALS.identity.sign_event(pre_event).await?;

    let relay_urls = relay::relays_to_post_to(&event)?;

    Ok(vec![(event, relay_urls)])
}

pub async fn prepare_post_nip17(
    author: PublicKey,
    content: String,
    mut tags: Vec<Tag>,
    dm_channel: DmChannel,
    annotation: bool,
) -> Result<Vec<(Event, Vec<RelayUrl>)>, Error> {
    if !dm_channel.can_use_nip17() {
        return Err(ErrorKind::UsersCantUseNip17.into());
    }

    let our_pk = match GLOBALS.identity.public_key() {
        Some(pk) => pk,
        None => return Err(ErrorKind::NoPublicKey.into()),
    };

    // Tags go onto the inner rumor:

    add_gossip_tag(&mut tags);

    add_tags_mirroring_content(&content, &mut tags, true).await;

    // All recipients get 'p' tagged on the DM rumor
    for pk in dm_channel.keys() {
        nostr_types::add_pubkey_to_tags(&mut tags, *pk, None);
    }

    // But we don't need (or want) the thread based tags.

    if annotation {
        tags.push(Tag::new(&["annotation"]))
    }

    let pre_event = PreEvent {
        pubkey: author,
        created_at: Unixtime::now(),
        kind: EventKind::DmChat,
        tags,
        content,
    };

    let mut output: Vec<(Event, Vec<RelayUrl>)> = Vec::new();

    // To all recipients
    for pk in dm_channel.keys() {
        let event = GLOBALS.identity.giftwrap(pre_event.clone(), *pk).await?;
        let relays = relay::get_dm_relays(*pk)?;
        output.push((event, relays));
    }

    // And a copy to us
    {
        let event = GLOBALS.identity.giftwrap(pre_event.clone(), our_pk).await?;
        let relays = Relay::choose_relay_urls(Relay::DM, |_| true)?;
        output.push((event, relays));
    }

    Ok(output)
}

fn add_gossip_tag(tags: &mut Vec<Tag>) {
    if GLOBALS.db().read_setting_set_client_tag() {
        tags.push(Tag::new(&["client", "gossip"]));
    }
}

async fn add_tags_mirroring_content(content: &str, tags: &mut Vec<Tag>, direct_message: bool) {
    let shattered_content = ShatteredContent::new(content.to_owned(), false);
    for segment in shattered_content.segments.iter() {
        match segment {
            ContentSegment::NostrUrl(nurl) => {
                match &nurl.0 {
                    NostrBech32::CryptSec(_) => {
                        // add nothing
                    }
                    NostrBech32::NAddr(ea) => {
                        // https://github.com/nostr-protocol/nips/pull/1560 may allow us to use 'q'
                        // in the future
                        nostr_types::add_addr_to_tags(tags, ea, Some("mention".to_string()));
                    }
                    NostrBech32::NEvent(ne) => {
                        // NIP-10: "Those marked with "mention" denote a quoted or reposted event id."
                        add_event_to_tags(
                            tags,
                            ne.id,
                            ne.relays.first().cloned(),
                            ne.author,
                            "mention", // this will use 'q', see the function
                        );
                    }
                    NostrBech32::Id(id) => {
                        // NIP-10: "Those marked with "mention" denote a quoted or reposted event id."
                        add_event_to_tags(tags, *id, None, None, "mention"); // this will use 'q', see the function
                    }
                    NostrBech32::Profile(prof) => {
                        if !direct_message {
                            nostr_types::add_pubkey_to_tags(tags, prof.pubkey, None);
                        }
                    }
                    NostrBech32::Pubkey(pk) => {
                        if !direct_message {
                            nostr_types::add_pubkey_to_tags(tags, *pk, None);
                        }
                    }
                    NostrBech32::Relay(_) => {
                        // we don't need to add this to tags I don't think.
                    }
                }
            }
            ContentSegment::TagReference(_index) => {
                // do nothing
            }
            ContentSegment::Hyperlink(span) => {
                if let Some(slice) = shattered_content.slice(span) {
                    if let Some(mimetype) = crate::media_url_mimetype(slice) {
                        add_imeta_tag(slice, mimetype, tags).await;
                    }
                }
            }
            ContentSegment::Plain(_span) => {
                // do nothing
            }
            ContentSegment::Hashtag(hashtag) => {
                tags.push(ParsedTag::Hashtag(hashtag.to_ascii_lowercase()).into_tag());
            }
        }
    }

    // Standardize nostr links (prepend 'nostr:' where missing)
    // (This was a bad idea to do this late in the process, it breaks links that contain
    //  nostr urls)
    // content = NostrUrl::urlize(&content);
}

async fn add_imeta_tag(urlstr: &str, mimetype: &str, tags: &mut Vec<Tag>) {
    //turn into a nostr_types::Url
    let url = match Url::try_from_str(urlstr) {
        Ok(url) => url,
        _ => return,
    };

    // Fetch the link and wait for it
    let use_cache = true;
    let bytes = match GLOBALS.fetcher.get(url.clone(), use_cache).await {
        Ok(FetchResult::Ready(b)) => b,
        _ => return,
    };

    // FIXME - in case we already have an imeta tag matching this url, we should
    //         find it, convert it into a FileMetadata, and delete it from tags to
    //         be replaced at the bottom of this function. However, I don't think
    //         it will ever happen so I'm just writing this note instead.

    let imeta = {
        let unchecked_url = url.to_unchecked_url();
        let mut imeta = FileMetadata::new(unchecked_url);

        imeta.m = Some(mimetype.to_owned());
        imeta.size = Some(bytes.len() as u64);

        let hash = {
            use sha2::Digest;
            let mut hasher = sha2::Sha256::new();
            hasher.update(&bytes);
            let result = hasher.finalize();
            hex::encode(result)
        };
        imeta.x = Some(hash);

        if mimetype.starts_with("image") {
            use image::{DynamicImage, GenericImageView};
            if let Ok(dynamic_image) = image::load_from_memory(&bytes) {
                let (w, h) = dynamic_image.dimensions();
                // Convert to RGBA8
                let dynamic_image = DynamicImage::ImageRgba8(dynamic_image.to_rgba8());
                if let Ok(blurhash) = blurhash::encode(
                    (4 * w / h).min(9),
                    (4 * h / w).min(9),
                    w,
                    h,
                    dynamic_image.as_bytes(),
                ) {
                    imeta.blurhash = Some(blurhash);
                    imeta.dim = Some((w as usize, h as usize));
                }
            }
        }

        imeta
    };

    tags.push(imeta.to_imeta_tag());
}

fn add_thread_based_tags(
    author: PublicKey,
    tags: &mut Vec<Tag>,
    parent: &Event,
) -> Result<(), Error> {
    // Add a 'p' tag for the author we are replying to (except if it is our own key)
    if parent.pubkey != author {
        nostr_types::add_pubkey_to_tags(tags, parent.pubkey, None);
    }

    // Add all the 'p' tags from the note we are replying to (except our own)
    // FIXME: Should we avoid taging people who are muted?
    for tag in &parent.tags {
        if let Ok(ParsedTag::Pubkey { pubkey, .. }) = tag.parse() {
            if pubkey != author {
                nostr_types::add_pubkey_to_tags(tags, pubkey, None);
            }
        }
    }

    let parent_relay: Option<UncheckedUrl> = GLOBALS
        .db()
        .get_event_seen_on_relay(parent.id)?
        .pop()
        .map(|(rurl, _)| rurl.to_unchecked_url());

    // Possibly add a tag to the 'root'
    let mut parent_is_root = true;
    match parent.replies_to_root() {
        Some(EventReference::Id {
            id: root,
            author,
            mut relays,
            marker: _,
        }) => {
            // Add an 'e' tag for the root
            add_event_to_tags(
                tags,
                root,
                relays.pop().map(|u| u.to_unchecked_url()),
                author,
                "root",
            );
            parent_is_root = false;
        }
        Some(EventReference::Addr(ea)) => {
            // Add an 'a' tag for the root
            nostr_types::add_addr_to_tags(tags, &ea, Some("root".to_string()));
            parent_is_root = false;
        }
        None => {
            // double check in case replies_to_root() isn't sufficient
            // (it might be but this code doesn't hurt)
            let ancestor = parent.replies_to();
            if ancestor.is_none() {
                // parent is the root
                add_event_to_tags(
                    tags,
                    parent.id,
                    parent_relay.clone(),
                    Some(parent.pubkey),
                    "root",
                );
            } else {
                parent_is_root = false;
            }
        }
    }

    // Add 'reply tags
    let reply_marker = if parent_is_root { "root" } else { "reply" };
    add_event_to_tags(
        tags,
        parent.id,
        parent_relay,
        Some(parent.pubkey),
        reply_marker,
    );
    if parent.kind.is_replaceable() {
        // Add an 'a' tag for the note we are replying to
        let d = parent.parameter().unwrap_or("".to_owned());
        nostr_types::add_addr_to_tags(
            tags,
            &NAddr {
                d,
                relays: vec![],
                kind: parent.kind,
                author: parent.pubkey,
            },
            Some(reply_marker.to_string()),
        );
    }

    // Possibly propagate a subject tag
    for tag in &parent.tags {
        if let Ok(ParsedTag::Subject(subject)) = tag.parse() {
            if !tags.iter().any(|t| t.tagname() == "subject") {
                let mut subject = subject.to_owned();
                if !subject.starts_with("Re: ") {
                    subject = format!("Re: {}", subject);
                }
                subject = subject.chars().take(80).collect();
                tags.push(ParsedTag::Subject(subject).into_tag());
            }
        }
    }

    Ok(())
}

fn add_event_to_tags(
    existing_tags: &mut Vec<Tag>,
    added: Id,
    relay_url: Option<UncheckedUrl>,
    opt_pubkey: Option<PublicKey>,
    marker: &str,
) -> usize {
    let relay_url = match relay_url {
        Some(url) => Some(url),
        None => relay::recommended_relay_hint(added)
            .ok()
            .flatten()
            .map(|rr| rr.to_unchecked_url()),
    };

    // We only use this for kind-1 so we always use_quote=true
    nostr_types::add_event_to_tags(existing_tags, added, relay_url, marker, opt_pubkey, true)
}

fn work_logger(work_receiver: mpsc::Receiver<u8>, powint: u8) {
    while let Ok(work) = work_receiver.recv() {
        if work >= powint {
            // Even if work > powint, it doesn't count since we declared our target.
            GLOBALS
                .status_queue
                .write()
                .write(format!("Message sent with {powint} bits of work computed."));
            break;
        } else {
            GLOBALS
                .status_queue
                .write()
                .write(format!("PoW: {work}/{powint}"));
        }
    }
}

fn copy_root_tags(tags: &mut Vec<Tag>, parent: &Event) -> usize {
    let mut root_tag_count: usize = 0;
    // Copy root tags (E, A, K, P) from the parent event
    for tag in &parent.tags {
        match tag.parse() {
            Ok(ParsedTag::RootEvent { .. })
            | Ok(ParsedTag::RootAddress { .. })
            | Ok(ParsedTag::RootKind { .. })
            | Ok(ParsedTag::RootPubkey { .. }) => {
                root_tag_count += 1;
                tags.push(tag.clone());
            }
            _ => {}
        }
    }

    root_tag_count
}

fn add_parent_tags(tags: &mut Vec<Tag>, parent: &Event, author: PublicKey) {
    let relay_hint = relay::recommended_relay_hint(parent.id)
        .ok()
        .flatten()
        .map(|rr| rr.to_unchecked_url());

    // k
    tags.push(ParsedTag::Kind(parent.kind).into_tag());

    // a
    if parent.kind.is_replaceable() {
        tags.push(
            ParsedTag::Address {
                address: NAddr {
                    d: parent.parameter().unwrap_or_default(),
                    relays: match relay_hint {
                        Some(ref h) => vec![h.clone()],
                        None => vec![],
                    },
                    kind: parent.kind,
                    author: parent.pubkey,
                },
                marker: None,
            }
            .into_tag(),
        );
    }

    // e
    tags.push(
        ParsedTag::Event {
            id: parent.id,
            recommended_relay_url: relay_hint.clone(),
            marker: None,
            author_pubkey: Some(parent.pubkey),
        }
        .into_tag(),
    );

    // p
    tags.push(
        ParsedTag::Pubkey {
            pubkey: parent.pubkey,
            recommended_relay_url: relay_hint.clone(),
            petname: None,
        }
        .into_tag(),
    );

    // Copy all 'p' tags from parent except our own pubkey
    // FIXME: Should we avoid taging people who are muted?
    for tag in &parent.tags {
        if let Ok(ParsedTag::Pubkey { pubkey, .. }) = tag.parse() {
            if pubkey != author {
                nostr_types::add_pubkey_to_tags(tags, pubkey, None);
            }
        }
    }

    // Possibly propagate a subject tag
    for tag in &parent.tags {
        if let Ok(ParsedTag::Subject(subject)) = tag.parse() {
            let mut subject = subject.to_owned();
            if !subject.starts_with("Re: ") {
                subject = format!("Re: {}", subject);
            }
            subject = subject.chars().take(80).collect();
            nostr_types::add_subject_to_tags_if_missing(tags, subject);
        }
    }
}

fn set_parent_as_root_tags(tags: &mut Vec<Tag>, parent: &Event) {
    let relay_hint = relay::recommended_relay_hint(parent.id)
        .ok()
        .flatten()
        .map(|rr| rr.to_unchecked_url());

    // k
    tags.push(ParsedTag::RootKind(parent.kind).into_tag());

    // a
    if parent.kind.is_replaceable() {
        tags.push(
            ParsedTag::RootAddress {
                address: NAddr {
                    d: parent.parameter().unwrap_or_default(),
                    relays: match relay_hint {
                        Some(ref h) => vec![h.clone()],
                        None => vec![],
                    },
                    kind: parent.kind,
                    author: parent.pubkey,
                },
                marker: None,
            }
            .into_tag(),
        );
    }

    // e
    tags.push(
        ParsedTag::RootEvent {
            id: parent.id,
            recommended_relay_url: relay_hint.clone(),
            marker: None,
            author_pubkey: Some(parent.pubkey),
        }
        .into_tag(),
    );

    // p
    tags.push(
        ParsedTag::RootPubkey {
            pubkey: parent.pubkey,
            recommended_relay_url: relay_hint.clone(),
            petname: None,
        }
        .into_tag(),
    );
}
