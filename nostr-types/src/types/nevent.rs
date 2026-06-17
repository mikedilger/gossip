use super::{EventKind, Id, PublicKey, UncheckedUrl};
use crate::Error;
use serde::{Deserialize, Serialize};
#[cfg(feature = "speedy")]
use speedy::{Readable, Writable};

/// An 'nevent': event id along with some relays in which that event may be found.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct NEvent {
    /// Event id
    pub id: Id,

    /// Some of the relays where this could be in
    pub relays: Vec<UncheckedUrl>,

    /// Kind (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub kind: Option<EventKind>,

    /// Author (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub author: Option<PublicKey>,
}

impl NEvent {
    /// Export as a bech32 encoded string ("nevent")
    pub fn as_bech32_string(&self) -> String {
        // Compose
        let mut tlv: Vec<u8> = Vec::new();

        // Push Id
        tlv.push(0); // the special value, in this case the id
        tlv.push(32); // the length of the value (always 32 for id)
        tlv.extend(self.id.0);

        // Push relays
        for relay in &self.relays {
            tlv.push(1); // type 'relay'
            let len = relay.0.len() as u8;
            tlv.push(len); // the length of the string
            tlv.extend(&relay.0.as_bytes()[..len as usize]);
        }

        // Maybe Push kind
        if let Some(kind) = self.kind {
            let kindnum: u32 = From::from(kind);
            let bytes = kindnum.to_be_bytes();
            tlv.push(3); // type 'kind'
            tlv.push(bytes.len() as u8); // '4'
            tlv.extend(bytes);
        }

        // Maybe Push author
        if let Some(pubkey) = self.author {
            tlv.push(2); // type 'author'
            tlv.push(32); // the length of the value (always 32 for public key)
            tlv.extend(pubkey.as_bytes());
        }

        bech32::encode::<bech32::Bech32>(*crate::HRP_NEVENT, &tlv).unwrap()
    }

    /// Import from a bech32 encoded string ("nevent")
    pub fn try_from_bech32_string(s: &str) -> Result<NEvent, Error> {
        let data = bech32::decode(s)?;
        if data.0 != *crate::HRP_NEVENT {
            Err(Error::WrongBech32(
                crate::HRP_NEVENT.to_lowercase(),
                data.0.to_lowercase(),
            ))
        } else {
            let mut relays: Vec<UncheckedUrl> = Vec::new();
            let mut id: Option<Id> = None;
            let mut kind: Option<EventKind> = None;
            let mut author: Option<PublicKey> = None;

            let tlv = data.1;
            let mut pos = 0;
            loop {
                // we need at least 2 more characters for anything meaningful
                if pos > tlv.len() - 2 {
                    break;
                }
                let ty = tlv[pos];
                let len = tlv[pos + 1] as usize;
                pos += 2;
                if pos + len > tlv.len() {
                    return Err(Error::InvalidProfile);
                }
                let raw = &tlv[pos..pos + len];
                match ty {
                    0 => {
                        // special (32 bytes of id)
                        if len != 32 {
                            return Err(Error::InvalidNEvent);
                        }
                        id = Some(Id(raw
                            .try_into()
                            .map_err(|_| Error::WrongLengthHexString)?));
                    }
                    1 => {
                        // relay
                        let relay_str = std::str::from_utf8(raw)?;
                        let relay = UncheckedUrl::from_str(relay_str);
                        relays.push(relay);
                    }
                    2 => {
                        // author
                        //
                        // Don't fail if the pubkey is bad, just don't include it.
                        // Some client is generating these, and we want to tolerate it
                        // as much as we can.
                        if let Ok(pk) = PublicKey::from_bytes(raw, true) {
                            author = Some(pk);
                        }
                    }
                    3 => {
                        // kind
                        let kindnum = u32::from_be_bytes(
                            raw.try_into().map_err(|_| Error::WrongLengthKindBytes)?,
                        );
                        kind = Some(kindnum.into());
                    }
                    _ => {} // unhandled type for nprofile
                }
                pos += len;
            }
            if let Some(id) = id {
                Ok(NEvent {
                    id,
                    relays,
                    kind,
                    author,
                })
            } else {
                Err(Error::InvalidNEvent)
            }
        }
    }

    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) fn mock() -> NEvent {
        let id = Id::try_from_hex_string(
            "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9",
        )
        .unwrap();

        NEvent {
            id,
            relays: vec![
                UncheckedUrl::from_str("wss://relay.example.com"),
                UncheckedUrl::from_str("wss://relay2.example.com"),
            ],
            kind: None,
            author: None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    test_serde! {NEvent, test_nevent_serde}

    #[test]
    fn test_profile_bech32() {
        let bech32 = NEvent::mock().as_bech32_string();
        println!("{bech32}");
        assert_eq!(
            NEvent::mock(),
            NEvent::try_from_bech32_string(&bech32).unwrap()
        );
    }

    #[test]
    fn test_nip19_example() {
        let nevent = NEvent {
            id: Id::try_from_hex_string(
                "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d",
            )
            .unwrap(),
            relays: vec![
                UncheckedUrl::from_str("wss://r.x.com"),
                UncheckedUrl::from_str("wss://djbas.sadkb.com"),
            ],
            kind: None,
            author: None,
        };

        // As serialized by us (not necessarily in the order others would do it)
        let bech32 = "nevent1qqsrhuxx8l9ex335q7he0f09aej04zpazpl0ne2cgukyawd24mayt8gpp4mhxue69uhhytnc9e3k7mgpz4mhxue69uhkg6nzv9ejuumpv34kytnrdaks343fay";

        // Try converting profile to bech32
        assert_eq!(nevent.as_bech32_string(), bech32);

        // Try converting bech32 to profile
        assert_eq!(nevent, NEvent::try_from_bech32_string(bech32).unwrap());

        // Try this one that used to fail
        let bech32 =
            "nevent1qqstxx3lk7zqfyn8cyyptvujfxq9w6mad4205x54772tdkmyqaay9scrqsqqqpp8x4vwhf";
        let _ = NEvent::try_from_bech32_string(bech32).unwrap();
        // it won't be equal, but should have the basics and should not error.
    }

    #[test]
    fn test_nevent_alt_fields() {
        let nevent = NEvent {
            id: Id::try_from_hex_string(
                "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d",
            )
            .unwrap(),
            relays: vec![
                UncheckedUrl::from_str("wss://r.x.com"),
                UncheckedUrl::from_str("wss://djbas.sadkb.com"),
            ],
            kind: Some(EventKind::TextNote),
            author: Some(
                PublicKey::try_from_hex_string(
                    "000000000332c7831d9c5a99f183afc2813a6f69a16edda7f6fc0ed8110566e6",
                    true,
                )
                .unwrap(),
            ),
        };

        // As serialized by us (not necessarily in the order others would do it)
        let bech32 = "nevent1qqsrhuxx8l9ex335q7he0f09aej04zpazpl0ne2cgukyawd24mayt8gpp4mhxue69uhhytnc9e3k7mgpz4mhxue69uhkg6nzv9ejuumpv34kytnrdaksxpqqqqqqzq3qqqqqqqqrxtrcx8vut2vlrqa0c2qn5mmf59hdmflkls8dsyg9vmnqu25v0j";

        // Try converting profile to bech32
        assert_eq!(nevent.as_bech32_string(), bech32);

        // Try converting bech32 to profile
        assert_eq!(nevent, NEvent::try_from_bech32_string(bech32).unwrap());
    }

    #[test]
    fn test_ones_that_were_failing() {
        let bech32 = "nevent1qqswrqr63ddwk8l3zfqrgdxh2lxh2jlcxl36k3h33g25gtchzchx8agpp4mhxue69uhkummn9ekx7mqpz3mhxue69uhhyetvv9ujuerpd46hxtnfduq3yamnwvaz7tm0venxx6rpd9hzuur4vgpyqdmyxs6rzdmyx4jxvdpnx4snjdmz8pnr2dtr8pnryefhv5ex2e34xvek2v3nxuckxef4v5ckxenxvs6njdtrxymnjcfnv4skvvekvs6qfe99uy";

        let _ne = NEvent::try_from_bech32_string(bech32).unwrap();
    }
}
