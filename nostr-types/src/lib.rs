// Copyright 2015-2020 nostr-proto Developers
// Licensed under the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

//! This crate provides types for nostr protocol handling.

#![deny(
    missing_debug_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unused_import_braces,
    //unused_qualifications,
    unused_results,
    unused_lifetimes,
    unused_labels,
    unused_extern_crates,
    non_ascii_idents,
    keyword_idents,
    deprecated_in_future,
    unstable_features,
    single_use_lifetimes,
    //unsafe_code,
    unreachable_pub,
    missing_docs,
    missing_copy_implementations
)]
#![deny(clippy::string_slice)]

mod error;
pub use error::Error;

#[cfg(test)]
macro_rules! test_serde {
    ($t:ty, $fnname:ident) => {
        #[test]
        fn $fnname() {
            let a = <$t>::mock();
            let x = serde_json::to_string(&a).unwrap();
            println!("{}", x);
            let b = serde_json::from_str(&x).unwrap();
            assert_eq!(a, b);
        }
    };
}

#[cfg(test)]
macro_rules! test_serde_async {
    ($t:ty, $fnname:ident) => {
        #[tokio::test]
        async fn $fnname() {
            let a = <$t>::mock().await;
            let x = serde_json::to_string(&a).unwrap();
            println!("{}", x);
            let b = serde_json::from_str(&x).unwrap();
            assert_eq!(a, b);
        }
    };
}

#[cfg(test)]
macro_rules! test_serde_val {
    ($fnname:ident, $val:expr) => {
        #[test]
        fn $fnname() {
            let a = $val;
            let x = serde_json::to_string(&a).unwrap();
            println!("{}", x);
            let b = serde_json::from_str(&x).unwrap();
            assert_eq!(a, b);
        }
    };
}

#[cfg(test)]
macro_rules! test_serde_val_async {
    ($fnname:ident, $val:expr) => {
        #[tokio::test]
        async fn $fnname() {
            let a = $val;
            let x = serde_json::to_string(&a).unwrap();
            println!("{}", x);
            let b = serde_json::from_str(&x).unwrap();
            assert_eq!(a, b);
        }
    };
}

/// A basic nostr client
#[cfg(feature = "client")]
pub mod client;

/// NIP-46 nsec bunker related types and functions
#[cfg(feature = "nip46")]
pub mod nip46;

mod types;
pub use types::{
    find_nostr_bech32_pos, find_nostr_url_pos, ClientMessage, ContentEncryptionAlgorithm,
    ContentSegment, CountResult, DelegationConditions, EncryptedPrivateKey, Event, EventDelegation,
    EventKind, EventKindIterator, EventKindOrRange, EventReference, ExportableSigner, Fee,
    FileMetadata, Filter, Hll8, Id, IdHex, Identity, KeySecurity, KeySigner, LockableSigner,
    Metadata, MilliSatoshi, MutExportableSigner, NAddr, NEvent, Nip05, NostrBech32, NostrUrl,
    ParsedTag, PayRequestData, PreEvent, PrivateKey, Profile, PublicKey, PublicKeyHex, RelayFees,
    RelayInformationDocument, RelayLimitation, RelayList, RelayListUsage, RelayMessage,
    RelayOrigin, RelayRetention, RelayUrl, RelayUsage, RelayUsageSet, Rumor, ShatteredContent,
    Signature, SignatureHex, Signer, SignerExt, SimpleRelayList, SimpleRelayUsage, Span,
    SubscriptionId, Tag, UncheckedUrl, Unixtime, Url, Why, XOnlyPublicKey, ZapData,
};

mod versioned;
pub use versioned::{
    EventV3, FeeV1, FilterV1, FilterV2, MetadataV1, MetadataV2, Nip05V1, PreEventV3, RelayFeesV1,
    RelayInformationDocumentV1, RelayInformationDocumentV2, RelayLimitationV1, RelayLimitationV2,
    RelayRetentionV1, RumorV3, TagV3, ZapDataV1, ZapDataV2,
};

#[inline]
pub(crate) fn get_leading_zero_bits(bytes: &[u8]) -> u8 {
    let mut res = 0_u8;
    for b in bytes {
        if *b == 0 {
            res += 8;
        } else {
            res += b.leading_zeros() as u8;
            return res;
        }
    }
    res
}

trait IntoVec<T> {
    fn into_vec(self) -> Vec<T>;
}

impl<T> IntoVec<T> for Option<T> {
    fn into_vec(self) -> Vec<T> {
        match self {
            None => vec![],
            Some(t) => vec![t],
        }
    }
}

use bech32::Hrp;
lazy_static::lazy_static! {
    static ref HRP_LNURL: Hrp = Hrp::parse("lnurl").expect("HRP error on lnurl");
    static ref HRP_NADDR: Hrp = Hrp::parse("naddr").expect("HRP error on naddr");
    static ref HRP_NCRYPTSEC: Hrp = Hrp::parse("ncryptsec").expect("HRP error on ncryptsec");
    static ref HRP_NEVENT: Hrp = Hrp::parse("nevent").expect("HRP error on nevent");
    static ref HRP_NOTE: Hrp = Hrp::parse("note").expect("HRP error on note");
    static ref HRP_NPROFILE: Hrp = Hrp::parse("nprofile").expect("HRP error on nprofile");
    static ref HRP_NPUB: Hrp = Hrp::parse("npub").expect("HRP error on npub");
    static ref HRP_NRELAY: Hrp = Hrp::parse("nrelay").expect("HRP error on nrelay");
    static ref HRP_NSEC: Hrp = Hrp::parse("nsec").expect("HRP error on nsec");
}

/// Add a 'p' pubkey tag to a set of tags if it doesn't already exist
pub fn add_pubkey_to_tags(
    existing_tags: &mut Vec<Tag>,
    new_pubkey: PublicKey,
    new_hint: Option<UncheckedUrl>,
) -> usize {
    let index = existing_tags.iter().position(|existing_tag| {
        if let Ok(ParsedTag::Pubkey { pubkey, .. }) = existing_tag.parse() {
            pubkey == new_pubkey
        } else {
            false
        }
    });

    if let Some(idx) = index {
        // force additional data to match
        existing_tags[idx].set_index(
            2,
            match new_hint {
                Some(u) => u.as_str().to_owned(),
                None => "".to_owned(),
            },
        );
        existing_tags[idx].trim();
        idx
    } else {
        existing_tags.push(
            ParsedTag::Pubkey {
                pubkey: new_pubkey,
                recommended_relay_url: new_hint,
                petname: None,
            }
            .into_tag(),
        );
        existing_tags.len() - 1
    }
}

/// Add an 'e' id tag to a set of tags if it doesn't already exist
pub fn add_event_to_tags(
    existing_tags: &mut Vec<Tag>,
    new_id: Id,
    new_hint: Option<UncheckedUrl>,
    new_marker: &str,
    new_pubkey: Option<PublicKey>,
    use_quote: bool,
) -> usize {
    // NIP-18: "Quote reposts are kind 1 events with an embedded q tag..."
    if new_marker == "mention" && use_quote {
        let index = existing_tags.iter().position(|existing_tag| {
            if let Ok(ParsedTag::Quote { id, .. }) = existing_tag.parse() {
                id == new_id
            } else {
                false
            }
        });

        if let Some(idx) = index {
            // force additional data to match
            existing_tags[idx].set_index(
                2,
                match new_hint {
                    Some(u) => u.as_str().to_owned(),
                    None => "".to_owned(),
                },
            );
            existing_tags[idx].set_index(
                3,
                match new_pubkey {
                    Some(pk) => pk.as_hex_string(),
                    None => "".to_owned(),
                },
            );
            existing_tags[idx].trim();
            idx
        } else {
            let newtag = ParsedTag::Quote {
                id: new_id,
                recommended_relay_url: new_hint,
                author_pubkey: new_pubkey,
            }
            .into_tag();
            existing_tags.push(newtag);
            existing_tags.len() - 1
        }
    } else {
        let index = existing_tags.iter().position(|existing_tag| {
            if let Ok(ParsedTag::Event { id, .. }) = existing_tag.parse() {
                id == new_id
            } else {
                false
            }
        });

        if let Some(idx) = index {
            // force additional data to match
            existing_tags[idx].set_index(
                2,
                match new_hint {
                    Some(u) => u.as_str().to_owned(),
                    None => "".to_owned(),
                },
            );
            existing_tags[idx].set_index(3, new_marker.to_owned());
            existing_tags[idx].set_index(
                4,
                match new_pubkey {
                    Some(pk) => pk.as_hex_string(),
                    None => "".to_owned(),
                },
            );
            existing_tags[idx].trim();
            idx
        } else {
            let newtag = ParsedTag::Event {
                id: new_id,
                recommended_relay_url: new_hint,
                marker: Some(new_marker.to_string()),
                author_pubkey: new_pubkey,
            }
            .into_tag();
            existing_tags.push(newtag);
            existing_tags.len() - 1
        }
    }
}

/// Add an 'a' addr tag to a set of tags if it doesn't already exist
pub fn add_addr_to_tags(
    existing_tags: &mut Vec<Tag>,
    new_addr: &NAddr,
    new_marker: Option<String>,
) -> usize {
    let index = existing_tags.iter().position(|existing_tag| {
        if let Ok(ParsedTag::Address { address, .. }) = existing_tag.parse() {
            address.kind == new_addr.kind
                && address.author == new_addr.author
                && address.d == new_addr.d
        } else {
            false
        }
    });

    if let Some(idx) = index {
        // force additional data to match
        existing_tags[idx].set_index(
            2,
            match new_marker {
                Some(s) => s,
                None => "".to_owned(),
            },
        );
        existing_tags[idx].trim();
        idx
    } else {
        existing_tags.push(
            ParsedTag::Address {
                address: new_addr.clone(),
                marker: new_marker,
            }
            .into_tag(),
        );
        existing_tags.len() - 1
    }
}

/// Add an 'subject' tag to a set of tags if it doesn't already exist
pub fn add_subject_to_tags_if_missing(existing_tags: &mut Vec<Tag>, subject: String) {
    if !existing_tags.iter().any(|t| t.tagname() == "subject") {
        existing_tags.push(ParsedTag::Subject(subject).into_tag());
    }
}
