use nostr_types::{PublicKey, Tag};

pub fn keys_from_text(text: &str) -> Vec<(String, PublicKey)> {
    let mut pubkeys: Vec<(String, PublicKey)> = text
        .split(&[' ', ',', '.', ':', ';', '?', '!', '/'][..])
        .filter_map(|npub| {
            if !npub.starts_with("npub1") {
                None
            } else {
                PublicKey::try_from_bech32_string(&npub)
                    .ok()
                    .map(|pubkey| (npub.to_string(), pubkey))
            }
        })
        .collect();
    pubkeys.sort_unstable_by_key(|nk| nk.1.as_bytes());
    pubkeys.dedup();
    pubkeys
}

pub fn add_pubkey_to_tags(existing_tags: &mut Vec<Tag>, added: PublicKey) -> usize {
    add_tag(
        existing_tags,
        &Tag::Pubkey {
            pubkey: added.as_hex_string().into(),
            recommended_relay_url: None,
            petname: None,
        },
    )
}

pub fn add_tag(existing_tags: &mut Vec<Tag>, added: &Tag) -> usize {
    match added {
        Tag::Pubkey { pubkey, .. } => {
            match existing_tags.iter().position(|existing_tag| {
                matches!(
                    existing_tag,
                    Tag::Pubkey { pubkey: existing_p, .. } if existing_p.0 == pubkey.0
                )
            }) {
                None => {
                    // add (FIXME: include relay hint it not exists)
                    existing_tags.push(added.to_owned());
                    existing_tags.len() - 1
                }
                Some(idx) => idx,
            }
        }
        Tag::Event { id, .. } => {
            match existing_tags.iter().position(|existing_tag| {
                matches!(
                    existing_tag,
                    Tag::Event { id: existing_e, .. } if existing_e.0 == id.0
                )
            }) {
                None => {
                    // add (FIXME: include relay hint it not exists)
                    existing_tags.push(added.to_owned());
                    existing_tags.len() - 1
                }
                Some(idx) => idx,
            }
        }
        Tag::Hashtag(hashtag) => {
            match existing_tags.iter().position(|existing_tag| {
                matches!(
                    existing_tag,
                    Tag::Hashtag(existing_hashtag) if existing_hashtag == hashtag
                )
            }) {
                None => {
                    // add (FIXME: include relay hint it not exists)
                    existing_tags.push(added.to_owned());
                    existing_tags.len() - 1
                }
                Some(idx) => idx,
            }
        }
        _ => {
            existing_tags.push(added.to_owned());
            existing_tags.len() - 1
        }
    }
}
