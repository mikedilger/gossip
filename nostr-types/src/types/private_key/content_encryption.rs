use super::{base64flex, PrivateKey};
use crate::{Error, PublicKey};
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use base64::Engine;
use rand_core::RngCore;
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

/// Content Encryption Algorithm
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ContentEncryptionAlgorithm {
    /// NIP-04 (insecure)
    Nip04,

    /// NIP-44 unpadded (produced by Amethyst for a few months around Aug-Oct 2023
    Nip44v1Unpadded,

    /// NIP-44 padded (possibly never in use, or a few tests were produced by Gossip around Aug-Oct 2023)
    Nip44v1Padded,

    /// NIP-44 v2 (latest, not yet audited)
    Nip44v2,
}

impl PrivateKey {
    /// Get the shared secret
    pub fn shared_secret(&self, other: &PublicKey, algo: ContentEncryptionAlgorithm) -> [u8; 32] {
        match algo {
            ContentEncryptionAlgorithm::Nip04 => self.shared_secret_nip04(other),
            ContentEncryptionAlgorithm::Nip44v1Unpadded => self.shared_secret_nip44_v1(other),
            ContentEncryptionAlgorithm::Nip44v1Padded => self.shared_secret_nip44_v1(other),
            ContentEncryptionAlgorithm::Nip44v2 => self.shared_secret_nip44_v2(other),
        }
    }

    /// Encrypt
    pub fn encrypt(
        &self,
        other: &PublicKey,
        plaintext: &str,
        algo: ContentEncryptionAlgorithm,
    ) -> Result<String, Error> {
        match algo {
            ContentEncryptionAlgorithm::Nip04 => self.nip04_encrypt(other, plaintext.as_bytes()),
            ContentEncryptionAlgorithm::Nip44v1Unpadded => {
                Ok(self.nip44_v1_encrypt(other, plaintext.as_bytes(), false, None))
            }
            ContentEncryptionAlgorithm::Nip44v1Padded => {
                Ok(self.nip44_v1_encrypt(other, plaintext.as_bytes(), true, None))
            }
            ContentEncryptionAlgorithm::Nip44v2 => self.nip44_v2_encrypt(other, plaintext),
        }
    }

    /// Detect encryption algorithm of ciphertext
    pub fn detect_encryption_algorithm(ciphertext: &str) -> ContentEncryptionAlgorithm {
        let cbytes = ciphertext.as_bytes();

        if cbytes.len() >= 28
            && cbytes[ciphertext.len() - 28] == b'?'
            && cbytes[ciphertext.len() - 27] == b'i'
            && cbytes[ciphertext.len() - 26] == b'v'
            && cbytes[ciphertext.len() - 25] == b'='
        {
            ContentEncryptionAlgorithm::Nip04
        } else {
            // We presume Nip44v1Unpadded and Nip44v1Padded are so rare and unused that
            // we won't find them here
            ContentEncryptionAlgorithm::Nip44v2
        }
    }

    /// Decrypt (detects encryption version)
    pub fn decrypt(&self, other: &PublicKey, ciphertext: &str) -> Result<String, Error> {
        if Self::detect_encryption_algorithm(ciphertext) == ContentEncryptionAlgorithm::Nip04 {
            self.decrypt_nip04(other, ciphertext)
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        } else {
            self.decrypt_nip44(other, ciphertext)
        }
    }

    /// Decrypt NIP-04 only
    pub fn decrypt_nip04(&self, other: &PublicKey, ciphertext: &str) -> Result<Vec<u8>, Error> {
        self.nip04_decrypt(other, ciphertext)
    }

    /// Decrypt NIP-44 only, version is detected
    pub fn decrypt_nip44(&self, other: &PublicKey, ciphertext: &str) -> Result<String, Error> {
        if ciphertext.as_bytes().first() == Some(&b'#') {
            return Err(nip44::Error::UnsupportedFutureVersion.into());
        };

        let algo = {
            let bytes = base64flex()
                .decode(ciphertext)
                .map_err(Error::BadEncryptedMessageBase64)?;
            if bytes.is_empty() {
                return Err(Error::BadEncryptedMessage);
            }
            match bytes[0] {
                1 => ContentEncryptionAlgorithm::Nip44v1Unpadded,
                // Note: Nip44v1Padded cannot be detected, and there may be no events out there using it.
                2 => ContentEncryptionAlgorithm::Nip44v2,
                _ => return Err(nip44::Error::UnknownVersion.into()),
            }
        };

        match algo {
            ContentEncryptionAlgorithm::Nip44v1Unpadded => {
                let bytes = self.nip44_v1_decrypt(other, ciphertext, false)?;
                Ok(String::from_utf8(bytes)?)
            }
            ContentEncryptionAlgorithm::Nip44v2 => self.nip44_v2_decrypt(other, ciphertext),
            _ => unreachable!(),
        }
    }

    /// Generate a shared secret with someone elses public key (NIP-04 method)
    fn shared_secret_nip04(&self, other: &PublicKey) -> [u8; 32] {
        // Build the whole PublicKey from the XOnlyPublicKey
        let pubkey = secp256k1::PublicKey::from_x_only_public_key(
            other.as_xonly_public_key(),
            secp256k1::Parity::Even,
        );

        // Get the shared secret point without hashing
        let mut shared_secret_point: [u8; 64] =
            secp256k1::ecdh::shared_secret_point(&pubkey, &self.0);

        // Take the first 32 bytes
        let mut shared_key: [u8; 32] = [0; 32];
        shared_key.copy_from_slice(&shared_secret_point[..32]);

        // Zeroize what we aren't keeping
        shared_secret_point.zeroize();

        shared_key
    }

    /// Generate a shared secret with someone elses public key (NIP-44 method, version 1)
    fn shared_secret_nip44_v1(&self, other: &PublicKey) -> [u8; 32] {
        // Build the whole PublicKey from the XOnlyPublicKey
        let pubkey = secp256k1::PublicKey::from_x_only_public_key(
            other.as_xonly_public_key(),
            secp256k1::Parity::Even,
        );

        let mut ssp = secp256k1::ecdh::shared_secret_point(&pubkey, &self.0)
            .as_slice()
            .to_owned();
        ssp.resize(32, 0); // keep only the X coordinate part

        let mut hasher = Sha256::new();
        hasher.update(ssp);
        let result = hasher.finalize();

        result.into()
    }

    /// Generate a shared secret with someone elses public key (NIP-44 method)
    fn shared_secret_nip44_v2(&self, other: &PublicKey) -> [u8; 32] {
        nip44::get_conversation_key(self.0, other.as_xonly_public_key())
    }

    /// Encrypt content via a shared secret according to NIP-04. Returns (IV, Ciphertext) pair.
    fn nip04_encrypt(&self, other: &PublicKey, plaintext: &[u8]) -> Result<String, Error> {
        let mut shared_secret = self.shared_secret_nip04(other);
        let iv = {
            let mut iv: [u8; 16] = [0; 16];
            rand::rng().fill_bytes(&mut iv);
            iv
        };

        let ciphertext = cbc::Encryptor::<aes::Aes256>::new(&shared_secret.into(), &iv.into())
            .encrypt_padded_vec_mut::<Pkcs7>(plaintext);

        shared_secret.zeroize();

        Ok(format!(
            "{}?iv={}",
            base64flex().encode(ciphertext),
            base64flex().encode(iv)
        ))
    }

    /// Decrypt content via a shared secret according to NIP-04
    fn nip04_decrypt(&self, other: &PublicKey, ciphertext: &str) -> Result<Vec<u8>, Error> {
        let parts: Vec<&str> = ciphertext.split("?iv=").collect();
        if parts.len() != 2 {
            return Err(Error::BadEncryptedMessage);
        }
        let ciphertext: Vec<u8> = base64flex()
            .decode(parts[0])
            .map_err(Error::BadEncryptedMessageBase64)?;
        let iv_vec: Vec<u8> = base64flex()
            .decode(parts[1])
            .map_err(Error::BadEncryptedMessageBase64)?;
        let iv: [u8; 16] = iv_vec.try_into().unwrap();

        let mut shared_secret = self.shared_secret_nip04(other);
        let plaintext = cbc::Decryptor::<aes::Aes256>::new(&shared_secret.into(), &iv.into())
            .decrypt_padded_vec_mut::<Pkcs7>(&ciphertext)?;

        shared_secret.zeroize();

        Ok(plaintext)
    }

    /// Encrypt content via a shared secret according to NIP-44 v1
    /// Always set forced_nonce=None (except for test vectors)
    fn nip44_v1_encrypt(
        &self,
        other: &PublicKey,
        plaintext: &[u8],
        pad: bool,
        forced_nonce: Option<[u8; 24]>,
    ) -> String {
        use rand::Rng;
        let mut new_plaintext;

        let encrypt = |plaintext: &[u8]| -> String {
            use chacha20::cipher::StreamCipher;
            let mut shared_secret = self.shared_secret_nip44_v1(other);
            let mut output: Vec<u8> = Vec::with_capacity(1 + 24 + plaintext.len());
            output.resize(1 + 24, 0);
            output[0] = 1; // Version
            match forced_nonce {
                Some(nonce) => output[1..=24].copy_from_slice(nonce.as_slice()),
                None => rand::rng().fill_bytes(&mut output[1..=24]),
            }
            output.extend(plaintext); // Plaintext (will encrypt in place)
            let mut cipher = chacha20::XChaCha20::new(&shared_secret.into(), output[1..=24].into());
            shared_secret.zeroize();
            cipher.apply_keystream(&mut output[25..]);
            base64flex().encode(output)
        };

        if pad {
            let end_plaintext = 4 + plaintext.len();

            // forced padding, up to a minimum of 32 bytes total so far (4 used for the u32 length)
            let forced_padding = 32_usize.saturating_sub(end_plaintext);
            let end_forced_padding = end_plaintext + forced_padding;

            // random length padding, up to 50% more
            let uniform = rand::distr::Uniform::new(0, end_forced_padding / 2).unwrap();
            let random_padding = rand::rng().sample(uniform);
            let end_random_padding = end_forced_padding + random_padding;

            // Make space
            new_plaintext = vec![0; end_random_padding];

            new_plaintext[0..4].copy_from_slice((plaintext.len() as u32).to_be_bytes().as_slice());
            new_plaintext[4..end_plaintext].copy_from_slice(plaintext);
            rand::rng().fill_bytes(&mut new_plaintext[end_plaintext..]); // random padding

            let output = encrypt(&new_plaintext);
            new_plaintext.zeroize();
            output
        } else {
            encrypt(plaintext)
        }
    }

    /// Decrypt content via a shared secret according to NIP-44, version 1
    fn nip44_v1_decrypt(
        &self,
        other: &PublicKey,
        ciphertext: &str,
        padded: bool,
    ) -> Result<Vec<u8>, Error> {
        use chacha20::cipher::StreamCipher;
        let mut shared_secret = self.shared_secret_nip44_v1(other);
        let bytes = base64flex()
            .decode(ciphertext)
            .map_err(Error::BadEncryptedMessageBase64)?;
        if bytes[0] != 1 {
            return Err(Error::UnknownCipherVersion(bytes[0]));
        }
        let mut output: Vec<u8> = Vec::with_capacity(bytes[25..].len());
        output.extend(&bytes[25..]);

        let mut cipher = chacha20::XChaCha20::new(&shared_secret.into(), bytes[1..=24].into());
        shared_secret.zeroize();
        cipher.apply_keystream(&mut output);

        if padded {
            let len = u32::from_be_bytes(output[0..4].try_into().unwrap());
            if 4 + len as usize > output.len() {
                return Err(Error::OutOfRange(len as usize));
            }
            Ok(output[4..4 + len as usize].to_owned())
        } else {
            Ok(output)
        }
    }

    /// Encrypt content via a shared secret according to NIP-44 v1
    fn nip44_v2_encrypt(&self, counterparty: &PublicKey, plaintext: &str) -> Result<String, Error> {
        let conversation_key = self.shared_secret_nip44_v2(counterparty);
        let ciphertext = nip44::encrypt(&conversation_key, plaintext)?;
        Ok(ciphertext)
    }

    /// Decrypt content via a shared secret according to NIP-44, version 2
    fn nip44_v2_decrypt(
        &self,
        counterparty: &PublicKey,
        ciphertext: &str,
    ) -> Result<String, Error> {
        let conversation_key = self.shared_secret_nip44_v2(counterparty);
        let plaintext = nip44::decrypt(&conversation_key, ciphertext)?;
        Ok(plaintext)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_privkey_nip04() {
        let private_key = PrivateKey::mock();
        let other_public_key = PublicKey::mock();

        let message = "hello world, this should come out just dandy.".as_bytes();
        let encrypted = private_key
            .nip04_encrypt(&other_public_key, message)
            .unwrap();
        let decrypted = private_key
            .nip04_decrypt(&other_public_key, &encrypted)
            .unwrap();
        assert_eq!(message, decrypted);
    }

    #[test]
    fn test_privkey_nip44_v1() {
        struct TestVector {
            sec1: &'static str,
            sec2: Option<&'static str>,
            pub2: Option<&'static str>,
            shared: Option<&'static str>,
            nonce: Option<&'static str>,
            plaintext: Option<Vec<u8>>,
            ciphertext: Option<&'static str>,
            note: &'static str,
            fail: bool,
        }

        impl Default for TestVector {
            fn default() -> TestVector {
                TestVector {
                    sec1: "0000000000000000000000000000000000000000000000000000000000000001",
                    sec2: None,
                    pub2: None,
                    shared: None,
                    nonce: None,
                    plaintext: None,
                    ciphertext: None,
                    note: "none",
                    fail: false,
                }
            }
        }

        let vectors: Vec<TestVector> = vec![
            TestVector {
                sec1: "0000000000000000000000000000000000000000000000000000000000000001",
                sec2: Some("0000000000000000000000000000000000000000000000000000000000000002"),
                shared: Some("0135da2f8acf7b9e3090939432e47684eb888ea38c2173054d4eedffdf152ca5"),
                nonce: Some("121f9d60726777642fd82286791ab4d7461c9502ebcbb6e6"),
                plaintext: Some(b"a".to_vec()),
                ciphertext: Some("ARIfnWByZ3dkL9gihnkatNdGHJUC68u25qM="),
                note: "sk1 = 1, sk2 = random, 0x02",
                .. Default::default()
            },
            TestVector {
                sec1: "0000000000000000000000000000000000000000000000000000000000000002",
                sec2: Some("0000000000000000000000000000000000000000000000000000000000000001"),
                shared: Some("0135da2f8acf7b9e3090939432e47684eb888ea38c2173054d4eedffdf152ca5"),
                plaintext: Some(b"a".to_vec()),
                ciphertext: Some("AeCt7jJ8L+WBOTiCSfeXEGXB/C/wgsrSRek="),
                nonce: Some("e0adee327c2fe58139388249f7971065c1fc2ff082cad245"),
                note: "sk1 = 1, sk2 = random, 0x02",
                .. Default::default()
            },
            TestVector {
                sec1: "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364139",
                pub2: Some("0000000000000000000000000000000000000000000000000000000000000002"),
                shared: Some("a6d6a2f7011cdd1aeef325948f48c6efa40f0ec723ae7f5ac7e3889c43481500"),
                nonce: Some("f481750e13dfa90b722b7cce0db39d80b0db2e895cc3001a"),
                plaintext: Some(b"a".to_vec()),
                ciphertext: Some("AfSBdQ4T36kLcit8zg2znYCw2y6JXMMAGjM="),
                note: "sec1 = n-2, pub2: random, 0x02",
                .. Default::default()
            },
            TestVector {
                sec1: "0000000000000000000000000000000000000000000000000000000000000002",
                pub2: Some("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdeb"),
                shared: Some("4908464f77dd74e11a9b4e4a3bc2467445bd794e8abcbfafb65a6874f9e25a8f"),
                nonce: Some("45c484ba2c0397853183adba6922156e09a2ad4e3e6914f2"),
                plaintext: Some(b"A Peer-to-Peer Electronic Cash System".to_vec()),
                ciphertext: Some("AUXEhLosA5eFMYOtumkiFW4Joq1OPmkU8k/25+3+VDFvOU39qkUDl1aiy8Q+0ozTwbhD57VJoIYayYS++hE="),
                note: "sec1 = 2, pub2: ",
                .. Default::default()
            },
            TestVector {
                sec1: "0000000000000000000000000000000000000000000000000000000000000001",
                pub2: Some("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"),
                shared: Some("132f39a98c31baaddba6525f5d43f2954472097fa15265f45130bfdb70e51def"),
                nonce: Some("d60de08405cf9bde508147e82224ac6af409c12b9e5492e1"),
                plaintext: Some(b"A purely peer-to-peer version of electronic cash would allow online payments to be sent directly from one party to another without going through a financial institution. Digital signatures provide part of the solution, but the main benefits are lost if a trusted third party is still required to prevent double-spending.".to_vec()),
                ciphertext: Some("AdYN4IQFz5veUIFH6CIkrGr0CcErnlSS4VdvoQaP2DCB1dIFL72HSriG1aFABcTlu86hrsG0MdOO9rPdVXc3jptMMzqvIN6tJlHPC8GdwFD5Y8BT76xIIOTJR2W0IdrM7++WC/9harEJAdeWHDAC9zNJX81CpCz4fnV1FZ8GxGLC0nUF7NLeUiNYu5WFXQuO9uWMK0pC7tk3XVogk90X6rwq0MQG9ihT7e1elatDy2YGat+VgQlDrz8ZLRw/lvU+QqeXMQgjqn42sMTrimG6NdKfHJSVWkT6SKZYVsuTyU1Iu5Nk0twEV8d11/MPfsMx4i36arzTC9qxE6jftpOoG8f/jwPTSCEpHdZzrb/CHJcpc+zyOW9BZE2ZOmSxYHAE0ustC9zRNbMT3m6LqxIoHq8j+8Ysu+Cwqr4nUNLYq/Q31UMdDg1oamYS17mWIAS7uf2yF5uT5IlG"),
                note: "sec1 == pub2",
                .. Default::default()
            },
            TestVector {
                sec1: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                pub2: Some("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"),
                plaintext: Some(b"a".to_vec()),
                note: "sec1 higher than curve.n",
                fail: true,
                .. Default::default()
            },
            TestVector {
                sec1: "0000000000000000000000000000000000000000000000000000000000000000",
                pub2: Some("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"),
                plaintext: Some(b"a".to_vec()),
                note: "sec1 is 0",
                fail: true,
                .. Default::default()
            },
            TestVector {
                sec1: "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364139",
                pub2: Some("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
                plaintext: Some(b"a".to_vec()),
                note: "pub2 is invalid, no sqrt, all-ff",
                fail: true,
                .. Default::default()
            },
            TestVector {
                sec1: "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141",
                pub2: Some("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"),
                plaintext: Some(b"a".to_vec()),
                note: "sec1 == curve.n",
                fail: true,
                .. Default::default()
            },
            TestVector {
                sec1: "0000000000000000000000000000000000000000000000000000000000000002",
                pub2: Some("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"),
                plaintext: Some(b"a".to_vec()),
                note: "pub2 is invalid, no sqrt",
                fail: true,
                .. Default::default()
            },
        ];

        for (num, vector) in vectors.iter().enumerate() {
            let mut sec1 = match PrivateKey::try_from_hex_string(vector.sec1) {
                Ok(key) => key,
                Err(_) => {
                    if vector.fail {
                        continue;
                    } else {
                        panic!("Test vector {} failed on sec1: {}", num, vector.note);
                    }
                }
            };
            println!("sec1: {}", sec1.as_hex_string());

            let pub2 = {
                if let Some(sec2) = vector.sec2 {
                    let sec2 = match PrivateKey::try_from_hex_string(sec2) {
                        Ok(priv_key) => priv_key,
                        Err(_) => {
                            if vector.fail {
                                continue;
                            } else {
                                panic!("Test vector {} failed on sec2: {}", num, vector.note);
                            }
                        }
                    };
                    sec2.public_key()
                } else if let Some(pub2) = vector.pub2 {
                    match PublicKey::try_from_hex_string(pub2, true) {
                        Ok(pub_key) => pub_key,
                        Err(_) => {
                            if vector.fail {
                                continue;
                            } else {
                                panic!("Test vector {} failed on pub2: {}", num, vector.note);
                            }
                        }
                    }
                } else {
                    panic!("Test vector {} has no sec2 or pub2: {}", num, vector.note);
                }
            };
            println!("pub2: {}", pub2.as_hex_string());

            // Test shared vector
            let shared = sec1.shared_secret_nip44_v1(&pub2);
            let shared_hex = hex::encode(shared);
            if let Some(s) = vector.shared {
                if s != shared_hex {
                    panic!(
                        "Test vector {} shared point mismatch: {}\ntheirs: {}\nours:   {}",
                        num, vector.note, s, shared_hex
                    );
                } else {
                    println!("Test vector {} shared point is good", num);
                }
            }

            // Test Encrypting
            if let (Some(plaintext), Some(ciphertext), Some(noncestr)) =
                (&vector.plaintext, vector.ciphertext, vector.nonce)
            {
                let nonce: [u8; 24] = hex::decode(noncestr).unwrap().try_into().unwrap();
                let ciphertext2 = sec1.nip44_v1_encrypt(&pub2, plaintext, false, Some(nonce));
                assert_eq!(ciphertext, ciphertext2);
                println!("Test vector {} encryption matches", num);
            }

            // Test Decrypting
            if let (Some(plaintext), Some(ciphertext), Some(sec2)) =
                (&vector.plaintext, vector.ciphertext, vector.sec2)
            {
                let sec2 = match PrivateKey::try_from_hex_string(sec2) {
                    Ok(key) => key,
                    Err(_) => {
                        if vector.fail {
                            continue;
                        } else {
                            panic!("Test vector {} failed on sec1: {}", num, vector.note);
                        }
                    }
                };
                let pub1 = sec1.public_key();

                let plaintext2 = sec2.nip44_v1_decrypt(&pub1, ciphertext, false).unwrap();
                assert_eq!(plaintext, &plaintext2);
                println!("Test vector {} decryption matches", num);
            }
        }
    }

    #[test]
    fn test_privkey_nip44_v1_pad() {
        let sec1 = PrivateKey::try_from_hex_string(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();

        let sec2 = PrivateKey::try_from_hex_string(
            "0000000000000000000000000000000000000000000000000000000000000002",
        )
        .unwrap();

        let plaintext = "yes".as_bytes();

        let ciphertext = sec1.nip44_v1_encrypt(&sec2.public_key(), plaintext, true, None);
        assert!(ciphertext.len() >= 32);

        let plaintext2 = sec2
            .nip44_v1_decrypt(&sec1.public_key(), &ciphertext, true)
            .unwrap();
        assert_eq!(plaintext, plaintext2);
    }

    #[test]
    fn test_nip44_version_detection() {
        let private_key = PrivateKey::generate();
        let private_key_b = PrivateKey::generate();
        let public_key = private_key_b.public_key();
        let message = "This is a test";

        let v1unpadded = private_key
            .encrypt(
                &public_key,
                message,
                ContentEncryptionAlgorithm::Nip44v1Unpadded,
            )
            .unwrap();
        let v1unpadded_decrypted = private_key.decrypt_nip44(&public_key, &v1unpadded).unwrap();

        assert_eq!(&v1unpadded_decrypted, message);

        let v2 = private_key
            .encrypt(&public_key, message, ContentEncryptionAlgorithm::Nip44v2)
            .unwrap();
        let v2_decrypted = private_key.decrypt_nip44(&public_key, &v2).unwrap();

        assert_eq!(&v2_decrypted, message);
    }
}
