mod client_message;
pub use client_message::ClientMessage;

mod content;
pub use content::{ContentSegment, ShatteredContent, Span};

mod delegation;
pub use delegation::{DelegationConditions, EventDelegation};

mod event;
pub use event::{Event, PreEvent, Rumor, ZapData};

mod event_kind;
pub use event_kind::{EventKind, EventKindIterator, EventKindOrRange};

mod event_reference;
pub use event_reference::EventReference;

mod file_metadata;
pub use file_metadata::FileMetadata;

mod filter;
pub use filter::Filter;

mod id;
pub use id::{Id, IdHex};

mod identity;
pub use identity::Identity;

mod key_signer;
pub use key_signer::KeySigner;

mod hll8;
pub use hll8::Hll8;

mod metadata;
pub use metadata::Metadata;

mod naddr;
pub use naddr::NAddr;

mod nevent;
pub use nevent::NEvent;

mod nip05;
pub use nip05::Nip05;

mod nostr_url;
pub use nostr_url::{find_nostr_bech32_pos, find_nostr_url_pos, NostrBech32, NostrUrl};

mod pay_request_data;
pub use pay_request_data::PayRequestData;

mod private_key;
pub use private_key::{ContentEncryptionAlgorithm, EncryptedPrivateKey, KeySecurity, PrivateKey};

mod profile;
pub use profile::Profile;

mod public_key;
pub use public_key::{PublicKey, PublicKeyHex, XOnlyPublicKey};

mod relay_information_document;
pub use relay_information_document::{
    Fee, RelayFees, RelayInformationDocument, RelayLimitation, RelayRetention,
};

mod relay_list;
pub use relay_list::{RelayList, RelayListUsage};

mod relay_message;
pub use relay_message::{CountResult, RelayMessage, Why};

mod relay_usage;
pub use relay_usage::{RelayUsage, RelayUsageSet};

mod satoshi;
pub use satoshi::MilliSatoshi;

mod signature;
pub use signature::{Signature, SignatureHex};

mod signer;
pub use signer::{ExportableSigner, LockableSigner, MutExportableSigner, Signer, SignerExt};

mod simple_relay_list;
pub use simple_relay_list::{SimpleRelayList, SimpleRelayUsage};

mod subscription_id;
pub use subscription_id::SubscriptionId;

mod tag;
pub use tag::{ParsedTag, Tag};

mod unixtime;
pub use unixtime::Unixtime;

mod url;
pub use self::url::{RelayOrigin, RelayUrl, UncheckedUrl, Url};

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn test_real_messages() {
        let wire = r#"["EVENT","j5happy-fiatjaf",{"id":"75468b04a0e03633a40f1c8d7e1a0cad1363ecc514ecbcde22093874e04e8166","pubkey":"3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d","created_at":1668011201,"kind":1,"tags":[["e","247baa8ed5db8097b16d9594a3a27fd2b64c030fa9e68ce7d6106df4a499700d","","reply"],["p","6b0d4c8d9dc59e110d380b0429a02891f1341a0fa2ba1b1cf83a3db4d47e3964","","reply"]],"content":"you're not allowed to pronounce these words, traitor","sig":"588577ccd5ad6be8f61d93e4738799dede9b169ad150ee3ee6a1c4bb80adfbee27bb4e302e0ea173637c189d6664f1dc82ad3590b5524240bf492fa0b754432c"}]"#;
        let message: RelayMessage = serde_json::from_str(wire).unwrap();
        match message {
            RelayMessage::Event(_subid, event) => {
                event.verify(None).unwrap();
                println!("{}", event.content);
            }
            _ => panic!("Wrong message type"),
        }

        let wire = r#"["EVENT","j5happy-fiatjaf",{"id":"267660849149c7226a4a4f7c75f359f3995965c05d25451f13c907bf0b158178","pubkey":"3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d","created_at":1668011264,"kind":1,"tags":[["e","8a128cd11c6a56554b8201635a19c97258504060464cec4f3e5f0500814339cf","","reply"],["p","000000000652e452ee68a01187fb08c899496cb46cb51d1aa0803d063acedba7","","reply"]],"content":"this is quite nice, specially the part where you say it was written in Rust.","sig":"1c49b4f4d2b86077ae4c1f7f8dc212d6c040dfdff7864eac2154fe7df1baceb162cf658d78634b803b964f920aeb861014ed30df113ed0857aaf1854e3c572a3"}]"#;
        let message: RelayMessage = serde_json::from_str(wire).unwrap();
        match message {
            RelayMessage::Event(_subid, event) => {
                event.verify(None).unwrap();
                println!("{}", event.as_ref().content);
            }
            _ => panic!("Wrong message type"),
        }

        let wire = r#"["EVENT","j5happy-fiatjaf",{"id":"fe0cfc6d2be988f46f849535518c3e43a509ea8a016ccd8b83a3ffd79575fd33","pubkey":"3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d","created_at":1668011340,"kind":1,"tags":[["e","b1a2a2e55f1b6f1f6756e6e4c1c4ecbce0123ede048423413228134143fd84ac","","root"],["e","c758d9d467bf925923f57bb6b47db870fad50ba9629bc086f573f3d4ff278c84","","reply"],["p","9ec7a778167afb1d30c4833de9322da0c08ba71a69e1911d5578d3144bb56437","","root"],["p","32e1827635450ebb3c5a7d12c1f8e7b2b514439ac10a67eef3d9fd9c5c68e245","","reply"]],"content":"they are definitely annoying in Go, but we already have them anyway because of the `[\"EVENT\", {}]` message so this doesn't make any difference in my case at least.","sig":"23b1eed3087a72f2e940c1c95541b22b3434390926780ed055abf5dd77a3aa16e1c5c3965382ec7343c0da3ece31e05945f910d684f3196e81e05765a5b1e631"}]"#;
        let message: RelayMessage = serde_json::from_str(wire).unwrap();
        match message {
            RelayMessage::Event(_subid, event) => {
                event.verify(None).unwrap();
                println!("{}", event.content);
            }
            _ => panic!("Wrong message type"),
        }

        let wire = r#"["EVENT","j5happy-fiatjaf",{"id":"adf038ca047260a20f70b7863c3a8ef7afdac455cd9fcb785950b86ebb104911","pubkey":"3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d","created_at":1668011516,"kind":1,"tags":[["e","c0138298e2ac89078e206aea1e16f1d9a37257c8400f48aba781dd890bc9f35b","","root"],["e","24b757dfc938d9d29d7be40ac91424bfecd8c0016929ac911447a2f785519d97","","reply"],["p","3235036bd0957dfb27ccda02d452d7c763be40c91a1ac082ba6983b25238388c","","root"],["p","46fcbe3065eaf1ae7811465924e48923363ff3f526bd6f73d7c184b16bd8ce4d","","reply"]],"content":"when I started writing branle a million years ago I thought it would be so much simpler too, I guess that explains why twitter has 800 developers on its payroll","sig":"0f7d1cfbcc38bb861f51538cb8e4a5268e2bdca13969eaba8d0993e19fa8469d9ebcc60081523d075ca63c7ab55270e2a3de2373db605cde081b82357907af1f"}]"#;
        let message: RelayMessage = serde_json::from_str(wire).unwrap();
        match message {
            RelayMessage::Event(_subid, event) => {
                event.verify(None).unwrap();
                println!("{}", event.content);
            }
            _ => panic!("Wrong message type"),
        }
    }
}
