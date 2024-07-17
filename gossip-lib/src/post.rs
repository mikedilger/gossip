use crate::dm_channel::DmChannel;
use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::relay;
use crate::relay::Relay;
use nostr_types::{
    ContentEncryptionAlgorithm, Event, EventAddr, EventKind, EventReference, Id, NostrBech32,
    PreEvent, PublicKey, RelayUrl, RelayUsage, Tag, UncheckedUrl, Unixtime,
};
use std::sync::mpsc;

pub fn prepare_post_normal(
    author: PublicKey,
    content: String,
    mut tags: Vec<Tag>,
    in_reply_to: Option<Id>,
    annotation: bool,
) -> Result<Vec<(Event, Vec<RelayUrl>)>, Error> {
    add_gossip_tag(&mut tags);

    add_tags_mirroring_content(&content, &mut tags, false);

    if let Some(parent_id) = in_reply_to {
        add_thread_based_tags(author, &mut tags, parent_id)?;
    }

    if annotation {
        tags.push(Tag::new(&["annotation"]))
    }

    let pre_event = PreEvent {
        pubkey: author,
        created_at: Unixtime::now(),
        kind: EventKind::TextNote,
        tags,
        content,
    };

    let event = {
        let powint = GLOBALS.storage.read_setting_pow();
        if powint > 0 {
            let (work_sender, work_receiver) = mpsc::channel();
            std::thread::spawn(move || {
                work_logger(work_receiver, powint);
            });
            GLOBALS
                .identity
                .sign_event_with_pow(pre_event, powint, Some(work_sender))?
        } else {
            GLOBALS.identity.sign_event(pre_event)?
        }
    };

    let mut relay_urls: Vec<RelayUrl> = Vec::new();
    relay_urls.extend({
        let tagged_pubkeys = get_tagged_pubkeys(&event.tags);
        relay::get_others_relays(&tagged_pubkeys, RelayUsage::Inbox)?
    });
    let our_relays = Relay::choose_relay_urls(Relay::WRITE, |_| true)?;
    relay_urls.extend(our_relays);

    relay_urls.sort();
    relay_urls.dedup();

    Ok(vec![(event, relay_urls)])
}

pub fn prepare_post_nip04(
    author: PublicKey,
    content: String,
    dm_channel: DmChannel,
    annotation: bool,
) -> Result<Vec<(Event, Vec<RelayUrl>)>, Error> {
    if dm_channel.keys().len() > 1 {
        return Err((ErrorKind::GroupDmsNotSupported, file!(), line!()).into());
    }

    let recipient = if dm_channel.keys().is_empty() {
        author // must be to yourself
    } else {
        dm_channel.keys()[0]
    };

    let content =
        GLOBALS
            .identity
            .encrypt(&recipient, &content, ContentEncryptionAlgorithm::Nip04)?;

    let mut tags = vec![Tag::new_pubkey(
        recipient, None, // FIXME
        None,
    )];
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

    let event = GLOBALS.identity.sign_event(pre_event)?;

    let mut relay_urls: Vec<RelayUrl> = Vec::new();
    relay_urls.extend({
        // Try DM relays first
        let mut relays = relay::get_dm_relays(recipient)?;
        if relays.is_empty() {
            // Fallback to their INBOX relays
            relays = relay::get_others_relays(&[recipient], RelayUsage::Inbox)?;
        }
        relays
    });
    let our_relays = Relay::choose_relay_urls(Relay::WRITE, |_| true)?;
    relay_urls.extend(our_relays);
    relay_urls.sort();
    relay_urls.dedup();

    Ok(vec![(event, relay_urls)])
}

pub fn prepare_post_nip17(
    author: PublicKey,
    content: String,
    mut tags: Vec<Tag>,
    dm_channel: DmChannel,
    annotation: bool,
) -> Result<Vec<(Event, Vec<RelayUrl>)>, Error> {
    if !dm_channel.can_use_nip17() {
        return Err(ErrorKind::UsersCantUseNip17.into());
    }

    let our_pk = match GLOBALS.storage.read_setting_public_key() {
        Some(pk) => pk,
        None => return Err(ErrorKind::NoPublicKey.into()),
    };

    // Tags go onto the inner rumor:

    add_gossip_tag(&mut tags);

    add_tags_mirroring_content(&content, &mut tags, true);

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
        let event = GLOBALS.identity.giftwrap(pre_event.clone(), *pk)?;
        let relays = relay::get_dm_relays(*pk)?;
        output.push((event, relays));
    }

    // And a copy to us
    {
        let event = GLOBALS.identity.giftwrap(pre_event.clone(), our_pk)?;
        let relays = Relay::choose_relay_urls(Relay::DM, |_| true)?;
        output.push((event, relays));
    }

    Ok(output)
}

fn get_tagged_pubkeys(tags: &[Tag]) -> Vec<PublicKey> {
    // Copy the tagged pubkeys for determine which relays to send to
    tags.iter()
        .filter_map(|t| {
            if let Ok((pubkey, _, _)) = t.parse_pubkey() {
                Some(pubkey)
            } else {
                None
            }
        })
        .collect()
}

fn add_gossip_tag(tags: &mut Vec<Tag>) {
    if GLOBALS.storage.read_setting_set_client_tag() {
        tags.push(Tag::new(&["client", "gossip"]));
    }
}

fn add_tags_mirroring_content(content: &str, tags: &mut Vec<Tag>, direct_message: bool) {
    // Add Tags based on references in the content
    //
    // FIXME - this function takes a 'tags' variable. We may want to let
    // the user determine which tags to keep and which to delete, so we
    // should probably move this processing into the post editor instead.
    // For now, I'm just trying to remove the old #[0] type substitutions
    // and use the new NostrBech32 parsing.
    for bech32 in NostrBech32::find_all_in_string(content).iter() {
        match bech32 {
            NostrBech32::EventAddr(ea) => {
                nostr_types::add_addr_to_tags(tags, ea, Some("mention".to_string()));
            }
            NostrBech32::EventPointer(ep) => {
                // NIP-10: "Those marked with "mention" denote a quoted or reposted event id."
                add_event_to_tags(tags, ep.id, ep.relays.first().cloned(), "mention");
            }
            NostrBech32::Id(id) => {
                // NIP-10: "Those marked with "mention" denote a quoted or reposted event id."
                add_event_to_tags(tags, *id, None, "mention");
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

    // Standardize nostr links (prepend 'nostr:' where missing)
    // (This was a bad idea to do this late in the process, it breaks links that contain
    //  nostr urls)
    // content = NostrUrl::urlize(&content);

    // Find and tag all hashtags
    for capture in GLOBALS.hashtag_regex.captures_iter(content) {
        tags.push(Tag::new_hashtag(capture[1][1..].to_string()));
    }
}

fn add_thread_based_tags(
    author: PublicKey,
    tags: &mut Vec<Tag>,
    parent_id: Id,
) -> Result<(), Error> {
    // Get the event we are replying to
    let parent = match GLOBALS.storage.read_event(parent_id)? {
        Some(e) => e,
        None => return Err("Cannot find event we are replying to.".into()),
    };

    // Add a 'p' tag for the author we are replying to (except if it is our own key)
    if parent.pubkey != author {
        nostr_types::add_pubkey_to_tags(tags, parent.pubkey, None);
    }

    // Add all the 'p' tags from the note we are replying to (except our own)
    // FIXME: Should we avoid taging people who are muted?
    for tag in &parent.tags {
        if let Ok((pubkey, _, _)) = tag.parse_pubkey() {
            if pubkey != author {
                nostr_types::add_pubkey_to_tags(tags, pubkey, None);
            }
        }
    }

    // Possibly add a tag to the 'root'
    let mut parent_is_root = true;
    match parent.replies_to_root() {
        Some(EventReference::Id {
            id: root,
            author: _,
            mut relays,
            marker: _,
        }) => {
            // Add an 'e' tag for the root
            add_event_to_tags(
                tags,
                root,
                relays.pop().map(|u| u.to_unchecked_url()),
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
                add_event_to_tags(tags, parent_id, None, "root");
            } else {
                parent_is_root = false;
            }
        }
    }

    // Add 'reply tags
    let reply_marker = if parent_is_root { "root" } else { "reply" };
    add_event_to_tags(tags, parent_id, None, reply_marker);
    if parent.kind.is_replaceable() {
        // Add an 'a' tag for the note we are replying to
        let d = parent.parameter().unwrap_or("".to_owned());
        nostr_types::add_addr_to_tags(
            tags,
            &EventAddr {
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
        if let Ok(subject) = tag.parse_subject() {
            let mut subject = subject.to_owned();
            if !subject.starts_with("Re: ") {
                subject = format!("Re: {}", subject);
            }
            subject = subject.chars().take(80).collect();
            nostr_types::add_subject_to_tags_if_missing(tags, subject);
        }
    }

    Ok(())
}

fn add_event_to_tags(
    existing_tags: &mut Vec<Tag>,
    added: Id,
    relay_url: Option<UncheckedUrl>,
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
    nostr_types::add_event_to_tags(existing_tags, added, relay_url, marker, true)
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
