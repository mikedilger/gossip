use nostr_types::{EventAddr, Id, PublicKey, Tag, UncheckedUrl};

use crate::relay::Relay;

pub async fn add_pubkey_to_tags(existing_tags: &mut Vec<Tag>, added: PublicKey) -> usize {
    let newtag = Tag::new_pubkey(added, None, None);

    match existing_tags.iter().position(|existing_tag| {
        if let Ok((pubkey, _, __)) = existing_tag.parse_pubkey() {
            pubkey == added
        } else {
            false
        }
    }) {
        None => {
            // FIXME: include relay hint
            existing_tags.push(newtag);
            existing_tags.len() - 1
        }
        Some(idx) => idx,
    }
}

// note - this is only used for kind-1 currently. If we change to other kinds,
// the 'q' tag        would currently be wrong.
pub async fn add_event_to_tags(
    existing_tags: &mut Vec<Tag>,
    added: Id,
    relay_url: Option<UncheckedUrl>,
    marker: &str,
) -> usize {
    let optrelay = match relay_url {
        Some(url) => Some(url),
        None => Relay::recommended_relay_for_reply(added)
            .await
            .ok()
            .flatten()
            .map(|rr| rr.to_unchecked_url()),
    };

    if marker == "mention" {
        // NIP-18: "Quote reposts are kind 1 events with an embedded q tag..."
        let newtag = Tag::new_quote(added, optrelay);

        match existing_tags.iter().position(|existing_tag| {
            if let Ok((id, _rurl)) = existing_tag.parse_quote() {
                id == added
            } else {
                false
            }
        }) {
            None => {
                existing_tags.push(newtag);
                existing_tags.len() - 1
            }
            Some(idx) => idx,
        }
    } else {
        let newtag = Tag::new_event(added, optrelay, Some(marker.to_string()));

        match existing_tags.iter().position(|existing_tag| {
            if let Ok((id, _rurl, _optmarker)) = existing_tag.parse_event() {
                id == added
            } else {
                false
            }
        }) {
            None => {
                existing_tags.push(newtag);
                existing_tags.len() - 1
            }
            Some(idx) => idx,
        }
    }
}

// FIXME pass in and set marker
pub async fn add_addr_to_tags(
    existing_tags: &mut Vec<Tag>,
    addr: &EventAddr,
    marker: Option<String>,
) -> usize {
    match existing_tags.iter().position(|existing_tag| {
        if let Ok((ea, _optmarker)) = existing_tag.parse_address() {
            ea.kind == addr.kind && ea.author == addr.author && ea.d == addr.d
        } else {
            false
        }
    }) {
        Some(idx) => idx,
        None => {
            existing_tags.push(Tag::new_address(addr, marker));
            existing_tags.len() - 1
        }
    }
}

pub fn add_subject_to_tags_if_missing(existing_tags: &mut Vec<Tag>, subject: String) {
    if !existing_tags.iter().any(|t| t.tagname() == "subject") {
        existing_tags.push(Tag::new_subject(subject));
    }
}

//#[cfg(test)]
// mod test {
//     use super::*;

//     #[test]
//     fn test_parse_pubkeys() {
//         let pubkeys = keys_from_text("hello
// npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6 and
// npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6... actually
// npub1melv683fw6n2mvhl5h6dhqd8mqfv3wmxnz4qph83ua4dk4006ezsrt5c24");
//         assert_eq!(pubkeys.len(), 2);
//         assert_eq!(
//             pubkeys[0].1.as_hex_string(),
//             
// "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d"         );
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
