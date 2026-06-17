use super::{EventKind, PublicKey, UncheckedUrl};
use crate::Error;
use serde::{Deserialize, Serialize};
#[cfg(feature = "speedy")]
use speedy::{Readable, Writable};
use std::hash::{Hash, Hasher};

/// An 'naddr': data to address a possibly parameterized replaceable event (d-tag, kind, author, and relays)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct NAddr {
    /// the 'd' tag of the Event, or an empty string if the kind is not parameterized
    pub d: String,

    /// Some of the relays where this could be found
    pub relays: Vec<UncheckedUrl>,

    /// Kind
    pub kind: EventKind,

    /// Author
    pub author: PublicKey,
}

impl NAddr {
    /// Export as a bech32 encoded string ("naddr")
    pub fn as_bech32_string(&self) -> String {
        // Compose
        let mut tlv: Vec<u8> = Vec::new();

        // Push d tag
        tlv.push(0); // the special value, in this case the 'd' tag
        let len = self.d.len() as u8;
        tlv.push(len); // the length of the d tag
        tlv.extend(&self.d.as_bytes()[..len as usize]);

        // Push relays
        for relay in &self.relays {
            tlv.push(1); // type 'relay'
            let len = relay.0.len() as u8;
            tlv.push(len); // the length of the string
            tlv.extend(&relay.0.as_bytes()[..len as usize]);
        }

        // Push kind
        let kindnum: u32 = From::from(self.kind);
        let bytes = kindnum.to_be_bytes();
        tlv.push(3); // type 'kind'
        tlv.push(bytes.len() as u8); // '4'
        tlv.extend(bytes);

        // Push author
        tlv.push(2); // type 'author'
        tlv.push(32); // the length of the value (always 32 for public key)
        tlv.extend(self.author.as_bytes());

        bech32::encode::<bech32::Bech32>(*crate::HRP_NADDR, &tlv).unwrap()
    }

    /// Import from a bech32 encoded string ("naddr")
    pub fn try_from_bech32_string(s: &str) -> Result<NAddr, Error> {
        let data = bech32::decode(s)?;
        if data.0 != *crate::HRP_NADDR {
            Err(Error::WrongBech32(
                crate::HRP_NADDR.to_lowercase(),
                data.0.to_lowercase(),
            ))
        } else {
            let mut maybe_d: Option<String> = None;
            let mut relays: Vec<UncheckedUrl> = Vec::new();
            let mut maybe_kind: Option<EventKind> = None;
            let mut maybe_author: Option<PublicKey> = None;

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
                        // special (bytes of d tag)
                        maybe_d = Some(std::str::from_utf8(raw)?.to_string());
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
                            maybe_author = Some(pk);
                        }
                    }
                    3 => {
                        // kind
                        let kindnum = u32::from_be_bytes(
                            raw.try_into().map_err(|_| Error::WrongLengthKindBytes)?,
                        );
                        maybe_kind = Some(kindnum.into());
                    }
                    _ => {} // unhandled type for nprofile
                }
                pos += len;
            }

            match (maybe_d, maybe_kind, maybe_author) {
                (Some(d), Some(kind), Some(author)) => {
                    if !kind.is_replaceable() {
                        Err(Error::NonReplaceableAddr)
                    } else {
                        Ok(NAddr {
                            d,
                            relays,
                            kind,
                            author,
                        })
                    }
                }
                _ => Err(Error::InvalidNAddr),
            }
        }
    }

    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) fn mock() -> NAddr {
        let d = "Test D Indentifier 1lkjf23".to_string();

        NAddr {
            d,
            relays: vec![
                UncheckedUrl::from_str("wss://relay.example.com"),
                UncheckedUrl::from_str("wss://relay2.example.com"),
            ],
            kind: EventKind::LongFormContent,
            author: PublicKey::mock_deterministic(),
        }
    }
}

impl PartialEq for NAddr {
    fn eq(&self, other: &Self) -> bool {
        self.d == other.d && self.kind == other.kind && self.author == other.author
        // We do not compare the relays field!
    }
}

impl Eq for NAddr {}

impl Hash for NAddr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.d.hash(state);
        self.kind.hash(state);
        self.author.hash(state);
        // We do not hash relays field!
    }
}

#[cfg(test)]
mod test {
    use super::*;

    test_serde! {NAddr, test_naddr_serde}

    #[test]
    fn test_profile_bech32() {
        let bech32 = NAddr::mock().as_bech32_string();
        println!("{bech32}");
        assert_eq!(
            NAddr::mock(),
            NAddr::try_from_bech32_string(&bech32).unwrap()
        );
    }
}
