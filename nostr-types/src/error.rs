use thiserror::Error;

/// Errors that can occur in the nostr-proto crate
#[derive(Error, Debug)]
pub enum Error {
    /// Assertion failed
    #[error("Assertion failed: {0}")]
    AssertionFailed(String),

    /// Bad NIP-46 Bunker URL
    #[error("Bad NIP-46 Bunker URL")]
    BadBunkerUrl,

    /// Bad Encrypted Message
    #[error("Bad Encrypted Message")]
    BadEncryptedMessage,

    /// Bad Encrypted Message due to bad Base64
    #[error("Bad Encrypted Message due to invalid base64")]
    BadEncryptedMessageBase64(base64::DecodeError),

    /// Base64 error
    #[error("Base64 Decoding Error: {0}")]
    Base64(#[from] base64::DecodeError),

    /// Bech32 decode error
    #[error("Bech32 Error: {0}")]
    Bech32Decode(#[from] bech32::DecodeError),

    /// Bech32 encode error
    #[error("Bech32 Error: {0}")]
    Bech32Encode(#[from] bech32::EncodeError),

    /// Bech32 HRP error
    #[error("Bech32 Error: {0}")]
    Bech32Hrp(#[from] bech32::primitives::hrp::Error),

    /// Crypto error
    #[error("Crypto Error: {0}")]
    Crypto(#[from] nip44::Error),

    /// Disconnected
    #[cfg(feature = "client")]
    #[error("Disconnected")]
    Disconnected,

    /// Encryption/Decryption Error
    #[error("Private Key Encryption/Decryption Error")]
    PrivateKeyEncryption,

    /// From utf8 Error
    #[error("From UTF-8 Error")]
    FromUtf8(#[from] std::string::FromUtf8Error),

    /// Bech32 error
    #[error("Wrong Bech32 Kind: Expected {0} found {0}")]
    WrongBech32(String, String),

    /// Key or Signature error
    #[error("Key or Signature Error: {0}")]
    KeyOrSignature(#[from] secp256k1::Error),

    /// Event is in the future
    #[error("Event is in the future")]
    EventInFuture,

    /// Formatting error
    #[error("Formatting Error: {0}")]
    Fmt(#[from] std::fmt::Error),

    /// A hash mismatch verification error
    #[error("Hash Mismatch")]
    HashMismatch,

    /// Hex string decoding error
    #[error("Hex Decode Error: {0}")]
    HexDecode(#[from] hex::FromHexError),

    /// HTTP error
    #[cfg(feature = "client")]
    #[error("HTTP: {0}")]
    Http(#[from] http::Error),

    /// Invalid encrypted private key
    #[error("Invalid Encrypted Private Key")]
    InvalidEncryptedPrivateKey,

    /// Invalid encrypted event
    #[error("Invalid Encrypted Event")]
    InvalidEncryptedEvent,

    /// Invalid HyperLogLog data
    #[error("Invalid HLL data")]
    InvalidHll,

    /// Invalid event Id
    #[error("Invalid event Id")]
    InvalidId,

    /// Invalid event Id Prefix
    #[error("Invalid event Id Prefix")]
    InvalidIdPrefix,

    /// Invalid digest length
    #[error("Invalid digest length")]
    InvalidLength(#[from] hmac::digest::InvalidLength),

    /// Invalid NAddr
    #[error("Invalid naddr")]
    InvalidNAddr,

    /// Invalid NEvent
    #[error("Invalid nevent")]
    InvalidNEvent,

    /// Invalid Operation
    #[error("Invalid Operation")]
    InvalidOperation,

    /// Invalid Private Key
    #[error("Invalid Private Key")]
    InvalidPrivateKey,

    /// Invalid Profile
    #[error("Invalid Profile")]
    InvalidProfile,

    /// Invalid public key
    #[error("Invalid Public Key")]
    InvalidPublicKey,

    /// Invalid public key prefix
    #[error("Invalid Public Key Prefix")]
    InvalidPublicKeyPrefix,

    /// Invalid recipient
    #[error("Invalid Recipient")]
    InvalidRecipient,

    /// Invalid state
    #[error("Invalid state: \"{0}\"")]
    InvalidState(String),

    /// Invalid URL
    #[error("Invalid URL: \"{0}\"")]
    InvalidUrl(#[from] url::ParseError),

    /// Invalid URI
    #[cfg(feature = "client")]
    #[error("Invalid URI: {0}")]
    InvalidUri(#[from] http::uri::InvalidUri),

    /// Invalid URL TLV encoding
    #[error("Invalid URL TLV encoding")]
    InvalidUrlTlv,

    /// Invalid URL Host
    #[error("Invalid URL Host: \"{0}\"")]
    InvalidUrlHost(String),

    /// Invalid URL Scheme
    #[error("Invalid URL Scheme: \"{0}\"")]
    InvalidUrlScheme(String),

    /// Missing URL Authority
    #[error("Missing URL Authority")]
    InvalidUrlMissingAuthority,

    /// NIP-46 error
    #[cfg(feature = "nip46")]
    #[error("NIP-46 error: {0}")]
    Nip46Error(String),

    /// NIP-46 failed to post
    #[cfg(feature = "nip46")]
    #[error("NIP-46 failed to post: {0}")]
    Nip46FailedToPost(String),

    /// NIP-46 failed to post
    #[cfg(feature = "nip46")]
    #[error("NIP-46 no response")]
    Nip46NoResponse,

    /// Addr to a non-replaceable event kind
    #[error("Event kind is not replaceable")]
    NonReplaceableAddr,

    /// No Private Key
    #[error("No private key")]
    NoPrivateKey,

    /// No Public Key
    #[error("No public key")]
    NoPublicKey,

    /// Out of Range
    #[error("Out of Range")]
    OutOfRange(usize),

    /// Parse integer error
    #[error("Parse integer error")]
    ParseInt(#[from] std::num::ParseIntError),

    /// Relay did not AUTH
    #[error("Relay (broken) says auth-required before challenging with AUTH")]
    RelayDidNotAuth,

    /// Relay forgot that we successfully AUTHed
    #[error("Relay (broken) forgot that we already successfully AUTHed")]
    RelayForgotAuth,

    /// Relay rejected our post
    #[error("Relay rejected our post")]
    RelayRejectedPost,

    /// Relay rejected our AUTH
    #[error("Relay rejected our AUTH")]
    RelayRejectedAuth,

    /// Relay requires AUTH but we aren't supplying it
    #[error("Relay requires AUTH")]
    RelayRequiresAuth,

    /// HTTP request eror
    #[cfg(feature = "client")]
    #[error("HTTP error: {0}")]
    Reqwest(#[from] reqwest::Error),

    /// Scrypt error
    #[error("Scrypt invalid output length")]
    Scrypt,

    /// Serialization error
    #[error("JSON (de)serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    /// Signer is locked
    #[error("Signer is locked")]
    SignerIsLocked,

    /// Try from slice error
    #[error("Try From Slice error: {0}")]
    Slice(#[from] std::array::TryFromSliceError),

    /// Speedy error
    #[cfg(feature = "speedy")]
    #[error("Speedy (de)serialization error: {0}")]
    Speedy(#[from] speedy::Error),

    /// Tag mismatch
    #[error("Tag mismatch")]
    TagMismatch,

    /// Timeout
    #[cfg(feature = "client")]
    #[error("Timeout")]
    TimedOut,

    /// Timeout
    #[cfg(feature = "client")]
    #[error("Timeout: {0}")]
    Timeout(#[from] tokio::time::error::Elapsed),

    /// Unknown event kind
    #[error("Unknown event kind = {0}")]
    UnknownEventKind(u32),

    /// Unknown Key Security
    #[error("Unknown key security = {0}")]
    UnknownKeySecurity(u8),

    /// Unknown Cipher Version
    #[error("Unknown cipher version = {0}")]
    UnknownCipherVersion(u8),

    /// Unpad error
    #[error("Decryption error: {0}")]
    Unpad(#[from] aes::cipher::block_padding::UnpadError),

    /// Unsupported Algorithm
    #[error("Unsupported algorithm")]
    UnsupportedAlgorithm,

    /// Url Error
    #[error("Not a valid nostr relay url: {0}")]
    Url(String),

    /// UTF-8 error
    #[error("UTF-8 Error: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),

    /// Websocket error
    #[cfg(feature = "client")]
    #[error("Websocket error: {0}")]
    Websocket(#[from] tungstenite::Error),

    /// Websocket Connection Failed
    #[cfg(feature = "client")]
    #[error("Websocket connection failed: {0}")]
    WebsocketConnectionFailed(http::StatusCode),

    /// Wrong event kind
    #[error("Wrong event kind")]
    WrongEventKind,

    /// Wrong length
    #[error("Wrong length")]
    WrongLength,

    /// Wrong length hex string
    #[error("Wrong length hex string")]
    WrongLengthHexString,

    /// Wrong length bytes for event kind
    #[error("Wrong length bytes for event kind")]
    WrongLengthKindBytes,

    /// Wrong Decryption Password
    #[error("Wrong decryption password")]
    WrongDecryptionPassword,

    /// Zap Receipt issue
    #[error("Invalid Zap Receipt: {0}")]
    ZapReceipt(String),
}
