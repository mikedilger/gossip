use super::{PublicKey, UncheckedUrl};
use crate::Error;
use serde::{Deserialize, Serialize};
#[cfg(feature = "speedy")]
use speedy::{Readable, Writable};

/// A person's profile on nostr which consists of the data needed in order to follow someone.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct Profile {
    /// Their public key
    pub pubkey: PublicKey,

    /// Some of the relays they post to (when the profile was created)
    pub relays: Vec<UncheckedUrl>,
}

impl Profile {
    /// Export as a bech32 encoded string ("nprofile")
    pub fn as_bech32_string(&self) -> String {
        // Compose
        let mut tlv: Vec<u8> = Vec::new();

        // Push Public Key
        tlv.push(0); // the special value, in this case the public key
        tlv.push(32); // the length of the value (always 32 for public key)
        tlv.extend(self.pubkey.as_slice());

        // Push relays
        for relay in &self.relays {
            tlv.push(1); // type 'relay'
            let len = relay.0.len() as u8;
            tlv.push(len); // the length of the string
            tlv.extend(&relay.0.as_bytes()[..len as usize]);
        }

        bech32::encode::<bech32::Bech32>(*crate::HRP_NPROFILE, &tlv).unwrap()
    }

    /// Import from a bech32 encoded string ("nprofile")
    ///
    /// If verify is true, will verify that it works as a secp256k1::XOnlyPublicKey. This
    /// has a performance cost.
    pub fn try_from_bech32_string(s: &str, verify: bool) -> Result<Profile, Error> {
        let data = bech32::decode(s)?;
        if data.0 != *crate::HRP_NPROFILE {
            Err(Error::WrongBech32(
                crate::HRP_NPROFILE.to_lowercase(),
                data.0.to_lowercase(),
            ))
        } else {
            let mut relays: Vec<UncheckedUrl> = Vec::new();
            let mut pubkey: Option<PublicKey> = None;
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
                match ty {
                    0 => {
                        // special,  32 bytes of the public key
                        if len != 32 {
                            return Err(Error::InvalidProfile);
                        }
                        pubkey = Some(PublicKey::from_bytes(&tlv[pos..pos + len], verify)?);
                    }
                    1 => {
                        // relay
                        let relay_bytes = &tlv[pos..pos + len];
                        let relay_str = std::str::from_utf8(relay_bytes)?;
                        let relay = UncheckedUrl::from_str(relay_str);
                        relays.push(relay);
                    }
                    _ => {} // unhandled type for nprofile
                }
                pos += len;
            }
            if let Some(pubkey) = pubkey {
                Ok(Profile { pubkey, relays })
            } else {
                Err(Error::InvalidProfile)
            }
        }
    }

    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) fn mock() -> Profile {
        let pubkey = PublicKey::try_from_hex_string(
            "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9",
            true,
        )
        .unwrap();

        Profile {
            pubkey,
            relays: vec![
                UncheckedUrl::from_str("wss://relay.example.com"),
                UncheckedUrl::from_str("wss://relay2.example.com"),
            ],
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    test_serde! {Profile, test_profile_serde}

    #[test]
    fn test_profile_bech32() {
        let bech32 = Profile::mock().as_bech32_string();
        println!("{bech32}");
        assert_eq!(
            Profile::mock(),
            Profile::try_from_bech32_string(&bech32, true).unwrap()
        );
    }

    #[test]
    fn test_nip19_example() {
        let profile = Profile {
            pubkey: PublicKey::try_from_hex_string(
                "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d",
                true,
            )
            .unwrap(),
            relays: vec![
                UncheckedUrl::from_str("wss://r.x.com"),
                UncheckedUrl::from_str("wss://djbas.sadkb.com"),
            ],
        };

        let bech32 = "nprofile1qqsrhuxx8l9ex335q7he0f09aej04zpazpl0ne2cgukyawd24mayt8gpp4mhxue69uhhytnc9e3k7mgpz4mhxue69uhkg6nzv9ejuumpv34kytnrdaksjlyr9p";

        // Try converting profile to bech32
        assert_eq!(profile.as_bech32_string(), bech32);

        // Try converting bech32 to profile
        assert_eq!(
            profile,
            Profile::try_from_bech32_string(bech32, true).unwrap()
        );
    }
}
