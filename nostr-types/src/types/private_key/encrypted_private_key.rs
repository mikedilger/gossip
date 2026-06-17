use super::{base64flex, KeySecurity, PrivateKey};
use crate::Error;
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use base64::Engine;
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, Payload},
    XChaCha20Poly1305,
};
use derive_more::Display;
use hmac::Hmac;
use pbkdf2::pbkdf2;
use rand_core::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
#[cfg(feature = "speedy")]
use speedy::{Readable, Writable};
use std::ops::Deref;
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroize;

// This allows us to detect bad decryptions with wrong passwords.
const V1_CHECK_VALUE: [u8; 11] = [15, 91, 241, 148, 90, 143, 101, 12, 172, 255, 103];
const V1_HMAC_ROUNDS: u32 = 100_000;

/// This is an encrypted private key (the string inside is the bech32 ncryptsec string)
#[derive(Clone, Debug, Display, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct EncryptedPrivateKey(pub String);

impl Deref for EncryptedPrivateKey {
    type Target = String;

    fn deref(&self) -> &String {
        &self.0
    }
}

impl EncryptedPrivateKey {
    /// Create from a bech32 string (this just type wraps as the internal stringly already is one)
    pub fn from_bech32_string(s: String) -> EncryptedPrivateKey {
        EncryptedPrivateKey(s)
    }

    /// only correct for version 1 and onwards
    pub fn as_bech32_string(&self) -> String {
        self.0.clone()
    }

    /// Decrypt into a Private Key with a passphrase.
    ///
    /// We recommend you zeroize() the password you pass in after you are
    /// done with it.
    pub fn decrypt(&self, password: &str) -> Result<PrivateKey, Error> {
        PrivateKey::import_encrypted(self, password)
    }

    /// Version
    ///
    /// Version -1:
    ///    PBKDF = pbkdf2-hmac-sha256 ( salt = "nostr", rounds = 4096 )
    ///    inside = concat(private_key, 15 specified bytes, key_security_byte)
    ///    encrypt = AES-256-CBC with random IV
    ///    compose = iv + ciphertext
    ///    encode = base64
    /// Version 0:
    ///    PBKDF = pbkdf2-hmac-sha256 ( salt = concat(0x1, 15 random bytes), rounds = 100000 )
    ///    inside = concat(private_key, 15 specified bytes, key_security_byte)
    ///    encrypt = AES-256-CBC with random IV
    ///    compose = salt + iv + ciphertext
    ///    encode = base64
    /// Version 1:
    ///    PBKDF = pbkdf2-hmac-sha256 ( salt = concat(0x1, 15 random bytes), rounds = 100000 )
    ///    inside = concat(private_key, 15 specified bytes, key_security_byte)
    ///    encrypt = AES-256-CBC with random IV
    ///    compose = salt + iv + ciphertext
    ///    encode = bech32('ncryptsec')
    /// Version 2:
    ///    PBKDF = scrypt ( salt = 16 random bytes, log_n = user choice, r = 8, p = 1)
    ///    inside = private_key
    ///    associated_data = key_security_byte
    ///    encrypt = XChaCha20-Poly1305
    ///    compose = concat (0x2, log_n, salt, nonce, associated_data, ciphertext)
    ///    encode = bech32('ncryptsec')
    pub fn version(&self) -> Result<i8, Error> {
        if self.0.starts_with("ncryptsec1") {
            let data = bech32::decode(&self.0)?;
            if data.0 != *crate::HRP_NCRYPTSEC {
                return Err(Error::WrongBech32(
                    crate::HRP_NCRYPTSEC.to_lowercase(),
                    data.0.to_lowercase(),
                ));
            }
            Ok(data.1[0] as i8)
        } else if self.0.len() == 64 {
            Ok(-1)
        } else {
            Ok(0) // base64 variant of v1
        }
    }
}

impl PrivateKey {
    /// Export in a (non-portable) encrypted form. This does not downgrade
    /// the security of the key, but you are responsible to keep it encrypted.
    /// You should not attempt to decrypt it, only use `import_encrypted()` on
    /// it, or something similar in another library/client which also respects key
    /// security.
    ///
    /// This currently exports into EncryptedPrivateKey version 2.
    ///
    /// We recommend you zeroize() the password you pass in after you are
    /// done with it.
    pub fn export_encrypted(
        &self,
        password: &str,
        log2_rounds: u8,
    ) -> Result<EncryptedPrivateKey, Error> {
        // Generate a random 16-byte salt
        let salt = {
            let mut salt: [u8; 16] = [0; 16];
            rand::rng().fill_bytes(&mut salt);
            salt
        };

        let mut rng = chacha20poly1305::aead::rand_core::OsRng;
        let nonce = XChaCha20Poly1305::generate_nonce(&mut rng);

        let associated_data: Vec<u8> = {
            let key_security: u8 = match self.1 {
                KeySecurity::Weak => 0,
                KeySecurity::Medium => 1,
                KeySecurity::NotTracked => 2,
            };
            vec![key_security]
        };

        let ciphertext = {
            let cipher = {
                let symmetric_key = Self::password_to_key_v2(password, &salt, log2_rounds)?;
                XChaCha20Poly1305::new((&symmetric_key).into())
            };

            // The inner secret. We don't have to drop this because we are encrypting-in-place
            let mut inner_secret: Vec<u8> = self.0.secret_bytes().to_vec();

            let payload = Payload {
                msg: &inner_secret,
                aad: &associated_data,
            };

            let ciphertext = match cipher.encrypt(&nonce, payload) {
                Ok(c) => c,
                Err(_) => return Err(Error::PrivateKeyEncryption),
            };

            inner_secret.zeroize();

            ciphertext
        };

        // Combine salt, IV and ciphertext
        let mut concatenation: Vec<u8> = Vec::new();
        concatenation.push(0x2); // 1 byte version number
        concatenation.push(log2_rounds); // 1 byte for scrypt N (rounds)
        concatenation.extend(salt); // 16 bytes of salt
        concatenation.extend(nonce); // 24 bytes of nonce
        concatenation.extend(associated_data); // 1 byte of key security
        concatenation.extend(ciphertext); // 48 bytes of ciphertext expected
                                          // Total length is 91 = 1 + 1 + 16 + 24 + 1 + 48

        // bech32 encode
        Ok(EncryptedPrivateKey(bech32::encode::<bech32::Bech32>(
            *crate::HRP_NCRYPTSEC,
            &concatenation,
        )?))
    }

    /// Import an encrypted private key which was exported with `export_encrypted()`.
    ///
    /// We recommend you zeroize() the password you pass in after you are
    /// done with it.
    ///
    /// This is backwards-compatible with keys that were exported with older code.
    pub fn import_encrypted(
        encrypted: &EncryptedPrivateKey,
        password: &str,
    ) -> Result<PrivateKey, Error> {
        if encrypted.0.starts_with("ncryptsec1") {
            // Versioned
            Self::import_encrypted_bech32(encrypted, password)
        } else {
            // Pre-versioned, deprecated
            Self::import_encrypted_base64(encrypted, password)
        }
    }

    // Current
    fn import_encrypted_bech32(
        encrypted: &EncryptedPrivateKey,
        password: &str,
    ) -> Result<PrivateKey, Error> {
        // bech32 decode
        let data = bech32::decode(&encrypted.0)?;
        if data.0 != *crate::HRP_NCRYPTSEC {
            return Err(Error::WrongBech32(
                crate::HRP_NCRYPTSEC.to_lowercase(),
                data.0.to_lowercase(),
            ));
        }
        match data.1[0] {
            1 => Self::import_encrypted_v1(data.1, password),
            2 => Self::import_encrypted_v2(data.1, password),
            _ => Err(Error::InvalidEncryptedPrivateKey),
        }
    }

    // current
    fn import_encrypted_v2(concatenation: Vec<u8>, password: &str) -> Result<PrivateKey, Error> {
        if concatenation.len() < 91 {
            return Err(Error::InvalidEncryptedPrivateKey);
        }

        // Break into parts
        let version: u8 = concatenation[0];
        assert_eq!(version, 2);
        let log2_rounds: u8 = concatenation[1];
        let salt: [u8; 16] = concatenation[2..2 + 16].try_into()?;
        let nonce = &concatenation[2 + 16..2 + 16 + 24];
        let associated_data = &concatenation[2 + 16 + 24..2 + 16 + 24 + 1];
        let ciphertext = &concatenation[2 + 16 + 24 + 1..];

        let cipher = {
            let symmetric_key = Self::password_to_key_v2(password, &salt, log2_rounds)?;
            XChaCha20Poly1305::new((&symmetric_key).into())
        };

        let payload = Payload {
            msg: ciphertext,
            aad: associated_data,
        };

        let mut inner_secret = match cipher.decrypt(nonce.into(), payload) {
            Ok(is) => is,
            Err(_) => return Err(Error::PrivateKeyEncryption),
        };

        if associated_data.is_empty() {
            return Err(Error::InvalidEncryptedPrivateKey);
        }
        let key_security = match associated_data[0] {
            0 => KeySecurity::Weak,
            1 => KeySecurity::Medium,
            2 => KeySecurity::NotTracked,
            _ => return Err(Error::InvalidEncryptedPrivateKey),
        };

        let signing_key =
            secp256k1::SecretKey::from_byte_array(inner_secret.as_slice().try_into().unwrap())?;
        inner_secret.zeroize();

        Ok(PrivateKey(signing_key, key_security))
    }

    // deprecated
    fn import_encrypted_base64(
        encrypted: &EncryptedPrivateKey,
        password: &str,
    ) -> Result<PrivateKey, Error> {
        let concatenation = base64flex().decode(&encrypted.0)?; // 64 or 80 bytes
        if concatenation.len() == 64 {
            Self::import_encrypted_pre_v1(concatenation, password)
        } else if concatenation.len() == 80 {
            Self::import_encrypted_v1(concatenation, password)
        } else {
            Err(Error::InvalidEncryptedPrivateKey)
        }
    }

    // deprecated
    fn import_encrypted_v1(concatenation: Vec<u8>, password: &str) -> Result<PrivateKey, Error> {
        // Break into parts
        let salt: [u8; 16] = concatenation[..16].try_into()?;
        let iv: [u8; 16] = concatenation[16..32].try_into()?;
        let ciphertext = &concatenation[32..]; // 48 bytes

        let key = Self::password_to_key_v1(password, &salt, V1_HMAC_ROUNDS)?;

        // AES-256-CBC decrypt
        let mut plaintext = cbc::Decryptor::<aes::Aes256>::new(&key.into(), &iv.into())
            .decrypt_padded_vec_mut::<Pkcs7>(ciphertext)?; // 44 bytes
        if plaintext.len() != 44 {
            return Err(Error::InvalidEncryptedPrivateKey);
            //return Err(Error::AssertionFailed("Import encrypted plaintext len != 44".to_owned()));
        }

        // Verify the check value
        if plaintext[plaintext.len() - 12..plaintext.len() - 1] != V1_CHECK_VALUE {
            return Err(Error::WrongDecryptionPassword);
        }

        // Get the key security
        let ks = KeySecurity::try_from(plaintext[plaintext.len() - 1])?;
        let output = PrivateKey(
            secp256k1::SecretKey::from_byte_array(
                plaintext[..plaintext.len() - 12].try_into().unwrap(),
            )?,
            ks,
        );

        // Here we zeroize plaintext:
        plaintext.zeroize();

        Ok(output)
    }

    // deprecated
    fn import_encrypted_pre_v1(
        iv_plus_ciphertext: Vec<u8>,
        password: &str,
    ) -> Result<PrivateKey, Error> {
        let key = Self::password_to_key_v1(password, b"nostr", 4096)?;

        if iv_plus_ciphertext.len() < 48 {
            // Should be 64 from padding, but we pushed in 48
            return Err(Error::InvalidEncryptedPrivateKey);
        }

        // Pull the IV off
        let iv: [u8; 16] = iv_plus_ciphertext[..16].try_into()?;
        let ciphertext = &iv_plus_ciphertext[16..]; // 64 bytes

        // AES-256-CBC decrypt
        let mut pt = cbc::Decryptor::<aes::Aes256>::new(&key.into(), &iv.into())
            .decrypt_padded_vec_mut::<Pkcs7>(ciphertext)?; // 48 bytes

        // Verify the check value
        if pt[pt.len() - 12..pt.len() - 1] != V1_CHECK_VALUE {
            return Err(Error::WrongDecryptionPassword);
        }

        // Get the key security
        let ks = KeySecurity::try_from(pt[pt.len() - 1])?;
        let output = PrivateKey(
            secp256k1::SecretKey::from_byte_array(pt[..pt.len() - 12].try_into().unwrap())?,
            ks,
        );

        // Here we zeroize pt:
        pt.zeroize();

        Ok(output)
    }

    // Hash/Stretch password with pbkdf2 into a 32-byte (256-bit) key
    fn password_to_key_v1(password: &str, salt: &[u8], rounds: u32) -> Result<[u8; 32], Error> {
        let mut key: [u8; 32] = [0; 32];
        pbkdf2::<Hmac<Sha256>>(password.as_bytes(), salt, rounds, &mut key)?;
        Ok(key)
    }

    // Hash/Stretch password with scrypt into a 32-byte (256-bit) key
    fn password_to_key_v2(password: &str, salt: &[u8; 16], log_n: u8) -> Result<[u8; 32], Error> {
        // Normalize unicode (NFKC)
        let password = password.nfkc().collect::<String>();

        let params = match scrypt::Params::new(log_n, 8, 1, 32) {
            // r=8, p=1
            Ok(p) => p,
            Err(_) => return Err(Error::Scrypt),
        };
        let mut key: [u8; 32] = [0; 32];
        if scrypt::scrypt(password.as_bytes(), salt, &params, &mut key).is_err() {
            return Err(Error::Scrypt);
        }
        Ok(key)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_export_import() {
        let pk = PrivateKey::generate();
        // we use a low log_n here because this is run slowly in debug mode
        let exported = pk.export_encrypted("secret", 13).unwrap();
        println!("{exported}");
        let imported_pk = PrivateKey::import_encrypted(&exported, "secret").unwrap();

        // Be sure the keys generate identical public keys
        assert_eq!(pk.public_key(), imported_pk.public_key());

        // Be sure the security level is still Medium
        assert_eq!(pk.key_security(), KeySecurity::Medium)
    }

    #[test]
    fn test_import_old_formats() {
        let decrypted = "a28129ab0b70c8d5e75aaf510ec00bff47fde7ca4ab9e3d9315c77edc86f037f";

        // pre-salt base64 (-2?)
        let encrypted = EncryptedPrivateKey("F+VYIvTCtIZn4c6owPMZyu4Zn5DH9T5XcgZWmFG/3ma4C3PazTTQxQcIF+G+daeFlkqsZiNIh9bcmZ5pfdRPyg==".to_owned());
        assert_eq!(
            encrypted.decrypt("nostr").unwrap().as_hex_string(),
            decrypted
        );

        // Version -1: post-salt base64
        let encrypted = EncryptedPrivateKey("AZQYNwAGULWyKweTtw6WCljV+1cil8IMRxfZ7Rs3nCfwbVQBV56U6eV9ps3S1wU7ieCx6EraY9Uqdsw71TY5Yv/Ep6yGcy9m1h4YozuxWQE=".to_owned());
        assert_eq!(
            encrypted.decrypt("nostr").unwrap().as_hex_string(),
            decrypted
        );

        let decrypted = "3501454135014541350145413501453fefb02227e449e57cf4d3a3ce05378683";

        // Version -1
        let encrypted = EncryptedPrivateKey("KlmfCiO+Tf8A/8bm/t+sXWdb1Op4IORdghC7n/9uk/vgJXIcyW7PBAx1/K834azuVmQnCzGq1pmFMF9rNPWQ9Q==".to_owned());
        assert_eq!(
            encrypted.decrypt("nostr").unwrap().as_hex_string(),
            decrypted
        );

        // Version 0:
        let encrypted = EncryptedPrivateKey("AZ/2MU2igqP0keoW08Z/rxm+/3QYcZn3oNbVhY6DSUxSDkibNp+bFN/WsRQxP7yBKwyEJVu/YSBtm2PI9DawbYOfXDqfmpA3NTPavgXwUrw=".to_owned());
        assert_eq!(
            encrypted.decrypt("nostr").unwrap().as_hex_string(),
            decrypted
        );

        // Version 1:
        let encrypted = EncryptedPrivateKey("ncryptsec1q9hnc06cs5tuk7znrxmetj4q9q2mjtccg995kp86jf3dsp3jykv4fhak730wds4s0mja6c9v2fvdr5dhzrstds8yks5j9ukvh25ydg6xtve6qvp90j0c8a2s5tv4xn7kvulg88".to_owned());
        assert_eq!(
            encrypted.decrypt("nostr").unwrap().as_hex_string(),
            decrypted
        );

        // Version 2:
        let encrypted = EncryptedPrivateKey("ncryptsec1qgg9947rlpvqu76pj5ecreduf9jxhselq2nae2kghhvd5g7dgjtcxfqtd67p9m0w57lspw8gsq6yphnm8623nsl8xn9j4jdzz84zm3frztj3z7s35vpzmqf6ksu8r89qk5z2zxfmu5gv8th8wclt0h4p".to_owned());
        assert_eq!(
            encrypted.decrypt("nostr").unwrap().as_hex_string(),
            decrypted
        );
    }

    #[test]
    fn test_nfkc_unicode_normalization() {
        // "ÅΩẛ̣"
        // U+212B U+2126 U+1E9B U+0323
        let password1: [u8; 11] = [
            0xE2, 0x84, 0xAB, 0xE2, 0x84, 0xA6, 0xE1, 0xBA, 0x9B, 0xCC, 0xA3,
        ];

        // "ÅΩẛ̣"
        // U+00C5 U+03A9 U+1E69
        let password2: [u8; 7] = [0xC3, 0x85, 0xCE, 0xA9, 0xE1, 0xB9, 0xA9];

        let password1_str = unsafe { std::str::from_utf8_unchecked(password1.as_slice()) };
        let password2_str = unsafe { std::str::from_utf8_unchecked(password2.as_slice()) };

        let password1_nfkc = password1_str.nfkc().collect::<String>();
        assert_eq!(password1_nfkc, password2_str);
    }
}

/*
 * version -1 (if 64 bytes, base64 encoded)
 *
 *    symmetric_aes_key = pbkdf2_hmac_sha256(password,  salt="nostr", rounds=4096)
 *    pre_encoded_encrypted_private_key = AES-256-CBC(IV=random, key=symmetric_aes_key, data=private_key)
 *    encrypted_private_key = base64(concat(IV, pre_encoded_encrypted_private_key))
 *
 * version 0 (80 bytes, base64 encoded, same as v1 internally)
 *
 *    symmetric_aes_key = pbkdf2_hmac_sha256(password,  salt=concat(0x1, 15 random bytes), rounds=100000)
 *    key_security_byte = 0x0 if weak, 0x1 if medium
 *    inner_concatenation = concat(
 *        private_key,                                         // 32 bytes
 *        [15, 91, 241, 148, 90, 143, 101, 12, 172, 255, 103], // 11 bytes
 *        key_security_byte                                    //  1 byte
 *    )
 *    pre_encoded_encrypted_private_key = AES-256-CBC(IV=random, key=symmetric_aes_key, data=private_key)
 *    outer_concatenation = concat(IV, pre_encoded_encrypted_private_key)
 *    encrypted_private_key = base64(outer_concatenation)
 *
 * version 1
 *
 *    salt = concat(byte(0x1), 15 random bytes)
 *    symmetric_aes_key = pbkdf2_hmac_sha256(password, salt=salt, rounds=100,000)
 *    key_security_byte = 0x0 if weak, 0x1 if medium
 *    inner_concatenation = concat(
 *        private_key,                                          // 32 bytes
 *        [15, 91, 241, 148, 90, 143, 101, 12, 172, 255, 103],  // 11 bytes
 *        key_security_byte                                     //  1 byte
 *    )
 *    pre_encoded_encrypted_private_key = AES-256-CBC(IV=random, key=symmetric_aes_key, data=private_key)
 *    outer_concatenation = concat(salt, IV, pre_encoded_encrypted_private_key)
 *    encrypted_private_key = bech32('ncryptsec', outer_concatenation)
 *
 * version 2 (scrypt, xchacha20-poly1305)
 *
 *    rounds = user selected power of 2
 *    salt = 16 random bytes
 *    symmetric_key = scrypt(password, salt=salt, r=8, p=1, N=rounds)
 *    key_security_byte = 0x0 if weak, 0x1 if medium, 0x2 if not implemented
 *    nonce = 12 random bytes
 *    pre_encoded_encrypted_private_key = xchacha20-poly1305(
 *        plaintext=private_key, nonce=nonce, key=symmetric_key,
 *        associated_data=key_security_byte
 *    )
 *    version = byte(0x3)
 *    outer_concatenation = concat(version, log2(rounds) as one byte, salt, nonce, pre_encoded_encrypted_private_key)
 *    encrypted_private_key = bech32('ncryptsec', outer_concatenation)
 */
