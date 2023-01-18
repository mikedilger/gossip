use crate::db::DbRelay;
use nostr_types::{Id, PublicKey, PublicKeyHex, Tag};

pub fn keys_from_text(text: &str) -> Vec<(String, PublicKey)> {
    let mut pubkeys: Vec<(String, PublicKey)> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter_map(|npub| {
            if !npub.starts_with("npub1") {
                None
            } else {
                PublicKey::try_from_bech32_string(npub)
                    .ok()
                    .map(|pubkey| (npub.to_string(), pubkey))
            }
        })
        .collect();
    pubkeys.sort_unstable_by_key(|nk| nk.1.as_bytes());
    pubkeys.dedup();
    pubkeys
}

pub fn notes_from_text(text: &str) -> Vec<(String, Id)> {
    let mut noteids: Vec<(String, Id)> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter_map(|note| {
            if !note.starts_with("note1") {
                None
            } else {
                Id::try_from_bech32_string(&note)
                    .ok()
                    .map(|id| (note.to_string(), id))
            }
        })
        .collect();
    noteids.sort_unstable_by_key(|ni| ni.1);
    noteids.dedup();
    noteids
}

pub async fn add_pubkey_hex_to_tags(existing_tags: &mut Vec<Tag>, hex: &PublicKeyHex) -> usize {
    let newtag = Tag::Pubkey {
        pubkey: hex.to_owned(),
        recommended_relay_url: None,
        petname: None,
    };

    match existing_tags.iter().position(|existing_tag| {
        matches!(
            existing_tag,
            Tag::Pubkey { pubkey: existing_p, .. } if existing_p.0 == hex.0
        )
    }) {
        None => {
            // FIXME: include relay hint
            existing_tags.push(newtag);
            existing_tags.len() - 1
        }
        Some(idx) => idx,
    }
}

pub async fn add_pubkey_to_tags(existing_tags: &mut Vec<Tag>, added: PublicKey) -> usize {
    add_pubkey_hex_to_tags(existing_tags, &added.as_hex_string().into()).await
}

pub async fn add_event_to_tags(existing_tags: &mut Vec<Tag>, added: Id, marker: &str) -> usize {
    let newtag = Tag::Event {
        id: added,
        recommended_relay_url: DbRelay::recommended_relay_for_reply(added)
            .await
            .ok()
            .flatten(),
        marker: Some(marker.to_string()),
    };

    match existing_tags.iter().position(|existing_tag| {
        matches!(
            existing_tag,
            Tag::Event { id: existing_e, .. } if existing_e.0 == added.0
        )
    }) {
        None => {
            existing_tags.push(newtag);
            existing_tags.len() - 1
        }
        Some(idx) => idx,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_pubkeys() {
        let pubkeys = keys_from_text("hello npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6 and npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6... actually npub1melv683fw6n2mvhl5h6dhqd8mqfv3wmxnz4qph83ua4dk4006ezsrt5c24");
        assert_eq!(pubkeys.len(), 2);
        assert_eq!(
            pubkeys[0].1.as_hex_string(),
            "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d"
        );
    }

    #[test]
    fn test_parse_notes() {
        let ids = notes_from_text(
            "note1pm88wxjcqfh886gf5tvzjwe6k0crmxzdwtfnmn7ww93dh8dcrkhq82j67f

Another naïve person falling for the scam of deletes.",
        );
        assert_eq!(ids.len(), 1);
    }
}
