use crate::db::DbRelay;
use nostr_types::{Id, PublicKey, PublicKeyHex, Tag};

pub async fn add_pubkey_hex_to_tags(existing_tags: &mut Vec<Tag>, hex: &PublicKeyHex) -> usize {
    let newtag = Tag::Pubkey {
        pubkey: hex.to_owned(),
        recommended_relay_url: None,
        petname: None,
    };

    match existing_tags.iter().position(|existing_tag| {
        matches!(
            existing_tag,
            Tag::Pubkey { pubkey: existing_p, .. } if existing_p == hex
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

pub async fn add_pubkey_to_tags(existing_tags: &mut Vec<Tag>, added: &PublicKey) -> usize {
    add_pubkey_hex_to_tags(existing_tags, &added.as_hex_string().into()).await
}

pub async fn add_event_to_tags(existing_tags: &mut Vec<Tag>, added: Id, marker: &str) -> usize {
    let newtag = Tag::Event {
        id: added,
        recommended_relay_url: DbRelay::recommended_relay_for_reply(added)
            .await
            .ok()
            .flatten()
            .map(|rr| rr.to_unchecked_url()),
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

pub fn add_subject_to_tags_if_missing(existing_tags: &mut Vec<Tag>, subject: String) {
    if !existing_tags.iter().any(|t| matches!(t, Tag::Subject(_))) {
        existing_tags.push(Tag::Subject(subject));
    }
}

//#[cfg(test)]
// mod test {
//     use super::*;

//     #[test]
//     fn test_parse_pubkeys() {
//         let pubkeys = keys_from_text("hello npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6 and npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6... actually npub1melv683fw6n2mvhl5h6dhqd8mqfv3wmxnz4qph83ua4dk4006ezsrt5c24");
//         assert_eq!(pubkeys.len(), 2);
//         assert_eq!(
//             pubkeys[0].1.as_hex_string(),
//             "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d"
//         );
//     }

//     #[test]
//     fn test_parse_notes() {
//         let ids = notes_from_text(
//             "note1pm88wxjcqfh886gf5tvzjwe6k0crmxzdwtfnmn7ww93dh8dcrkhq82j67f

// Another naïve person falling for the scam of deletes.",
//         );
//         assert_eq!(ids.len(), 1);
//     }
// }
