use crate::comms::{ToMinionMessage, ToOverlordMessage};
use crate::people::PersonList;
use nostr_types::RelayUrl;

/// Error kinds that can occur in gossip-lib
#[derive(Debug)]
pub enum ErrorKind {
    BadNostrConnectString,
    BroadcastSend(String),
    BroadcastReceive(tokio::sync::broadcast::error::RecvError),
    CannotUpdateRelayUrl,
    Delegation(String),
    Empty(String),
    EmptyJob,
    EngageDisallowed,
    EngagePending,
    EventNotFound,
    FromUtf8(std::string::FromUtf8Error),
    General(String),
    GroupDmsNotSupported,
    HttpError(http::Error),
    JoinError(tokio::task::JoinError),
    KeySizeWrong,
    Lmdb(heed::Error),
    MaxRelaysReached,
    MpscSend(tokio::sync::mpsc::error::SendError<ToOverlordMessage>),
    Nip05KeyNotFound,
    Nip46CommandMissingId,
    Nip46CommandNotJsonObject,
    Nip46Denied,
    Nip46NeedApproval,
    Nip46ParsingError(String, String),
    Nip46RelayNeeded,
    Nostr(nostr_types::Error),
    NoPublicKey,
    NoPrivateKey,
    NoPrivateKeyForAuth(RelayUrl),
    NoRelay,
    NotAPersonListEvent,
    NoSlotsRemaining,
    Image(image::error::ImageError),
    ImageFailure,
    Io(std::io::Error),
    Internal(String),
    InvalidFilter,
    InvalidUriParts(http::uri::InvalidUriParts),
    InvalidDnsId,
    InvalidUri(http::uri::InvalidUri),
    InvalidUrl(String),
    ListAllocationFailed,
    ListAlreadyExists(PersonList),
    ListEventMissingDtag,
    ListIsNotEmpty,
    ListIsWellKnown,
    ListNotFound,
    LoadMoreFailed,
    NoRelays,
    NoPeopleLeft,
    NoProgress,
    NostrConnectNotSetup,
    Offline,
    ParseInt(std::num::ParseIntError),
    ParseBool(std::str::ParseBoolError),
    RecordIsNotNewable,
    Regex(regex::Error),
    RelayRejectedUs,
    ReqwestHttpError(reqwest::Error),
    SerdeJson(serde_json::Error),
    ShuttingDown,
    SliceError(std::array::TryFromSliceError),
    Speedy(speedy::Error),
    Svg(usvg::Error),
    TagNotIndexed(String),
    Timeout(tokio::time::error::Elapsed),
    UnknownCommand(String),
    UnsupportedRelayUsage,
    UrlHasEmptyHostname,
    UrlHasNoHostname,
    UrlParse(url::ParseError),
    Usage(String, String), // error, usage line
    UsersCantUseNip17,
    Utf8Error(std::str::Utf8Error),
    Websocket(tungstenite::Error),
    WrongEventKind,
}

/// Errors that can occur in gossip-lib, optionally including a file and line number
/// where they were generated
#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
    pub file: Option<&'static str>,
    pub line: Option<u32>,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ErrorKind::*;
        if let Some(file) = self.file {
            write!(f, "{file}:")?;
        }
        if let Some(line) = self.line {
            write!(f, "{line}:")?;
        }
        match &self.kind {
            BadNostrConnectString => write!(f, "Bad nostrconnect string"),
            BroadcastSend(s) => write!(f, "Error broadcasting: {s}"),
            BroadcastReceive(e) => write!(f, "Error receiving broadcast: {e}"),
            CannotUpdateRelayUrl => {
                write!(f, "Cannot update relay url (create a new relay instead)")
            }
            Delegation(s) => write!(f, "NIP-26 Delegation Error: {s}"),
            Empty(s) => write!(f, "{s} is empty"),
            EmptyJob => write!(f, "relay job is empty"),
            EngageDisallowed => write!(f, "relay is disallowed"),
            EngagePending => write!(f, "relay approval is pending"),
            EventNotFound => write!(f, "Event not found"),
            FromUtf8(e) => write!(f, "UTF-8 error: {e}"),
            GroupDmsNotSupported => write!(f, "Group DMs are not supported under NIP-04"),
            General(s) => write!(f, "{s}"),
            HttpError(e) => write!(f, "HTTP error: {e}"),
            JoinError(e) => write!(f, "Task join error: {e}"),
            KeySizeWrong => write!(f, "Key size is wrong"),
            Lmdb(e) => write!(f, "LMDB: {e}"),
            MaxRelaysReached => write!(
                f,
                "Maximum relay connections reached, will not connect to another"
            ),
            MpscSend(e) => write!(f, "Error sending mpsc: {e}"),
            Nip05KeyNotFound => write!(f, "NIP-05 public key not found"),
            Nip46CommandMissingId => write!(f, "NIP-46 command missing ID"),
            Nip46CommandNotJsonObject => write!(f, "NIP-46 command not a json object"),
            Nip46Denied => write!(f, "NIP-46 command denied"),
            Nip46NeedApproval => write!(f, "NIP-46 approval needed"),
            Nip46ParsingError(_id, e) => write!(f, "NIP-46 parse error: {e}"),
            Nip46RelayNeeded => write!(f, "NIP-46 relay needed to respond."),
            Nostr(e) => write!(f, "Nostr: {e}"),
            NoPublicKey => write!(f, "No public key identity available."),
            NoPrivateKey => write!(f, "No private key available."),
            NoPrivateKeyForAuth(u) => {
                write!(f, "No private key available, cannot AUTH to relay: {}", u)
            }
            NoRelay => write!(f, "Could not determine a relay to use."),
            NotAPersonListEvent => write!(f, "Not a person list event"),
            NoSlotsRemaining => write!(f, "No custom list slots remaining."),
            Image(e) => write!(f, "Image: {e}"),
            ImageFailure => write!(f, "Image Failure"),
            Io(e) => write!(f, "I/O Error: {e}"),
            Internal(s) => write!(f, "INTERNAL: {s}"),
            InvalidFilter => write!(f, "Invalid filter"),
            InvalidUriParts(e) => write!(f, "Invalid URI parts: {e}"),
            InvalidDnsId => write!(f, "Invalid DNS ID (nip-05), should be user@domain"),
            InvalidUri(e) => write!(f, "Invalid URI: {e}"),
            InvalidUrl(s) => write!(f, "Invalid URL: {s}"),
            ListAllocationFailed => write!(f, "List allocation failed (no more slots)"),
            ListAlreadyExists(_) => write!(f, "List already exists"),
            ListEventMissingDtag => write!(f, "List event missing d-tag"),
            ListIsNotEmpty => write!(f, "List is not empty"),
            ListIsWellKnown => write!(f, "List is well known and cannot be deallocated"),
            ListNotFound => write!(f, "List was not found"),
            LoadMoreFailed => write!(f, "Load more failed"),
            NoRelays => write!(f, "No relays"),
            NoPeopleLeft => write!(f, "No people left"),
            NoProgress => write!(f, "No progress"),
            NostrConnectNotSetup => write!(f, "NostrConnect not setup, cannot connect"),
            Offline => write!(f, "Offline"),
            ParseInt(e) => write!(f, "Bad integer: {e}"),
            ParseBool(e) => write!(f, "Bad bool: {e}"),
            RecordIsNotNewable => write!(f, "Record is not newable"),
            Regex(e) => write!(f, "Regex: {e}"),
            RelayRejectedUs => write!(f, "Relay rejected us."),
            ReqwestHttpError(e) => write!(f, "HTTP (reqwest) error: {e}"),
            SerdeJson(e) => write!(f, "SerdeJson Error: {e}"),
            ShuttingDown => write!(f, "Shutting down"),
            SliceError(e) => write!(f, "Slice: {e}"),
            Speedy(e) => write!(f, "Speedy: {e}"),
            Svg(e) => write!(f, "SVG: {e}"),
            TagNotIndexed(s) => write!(f, "Tag not indexed: {s}"),
            Timeout(e) => write!(f, "Timeout: {e}"),
            UnknownCommand(s) => write!(f, "Unknown command: {s}"),
            UnsupportedRelayUsage => write!(f, "Unsupported relay usage"),
            UrlHasEmptyHostname => write!(f, "URL has empty hostname"),
            UrlHasNoHostname => write!(f, "URL has no hostname"),
            UrlParse(e) => write!(f, "URL parse: {e}"),
            Usage(e, u) => write!(f, "{}\n\nUsage: {}", e, u),
            UsersCantUseNip17 => write!(f, "User(s) can't use NIP-17 DMs"),
            Utf8Error(e) => write!(f, "UTF-8 error: {e}"),
            Websocket(e) => write!(f, "Websocket: {e}"),
            WrongEventKind => write!(f, "Wrong event kind"),
        }
    }
}

impl<E> From<(E, &'static str, u32)> for Error
where
    ErrorKind: From<E>,
{
    fn from(triplet: (E, &'static str, u32)) -> Error {
        Error {
            kind: triplet.0.into(),
            file: Some(triplet.1),
            line: Some(triplet.2),
        }
    }
}

impl<E> From<E> for Error
where
    ErrorKind: From<E>,
{
    fn from(intoek: E) -> Error {
        Error {
            kind: intoek.into(),
            file: None,
            line: None,
        }
    }
}

impl From<tokio::sync::broadcast::error::SendError<ToMinionMessage>> for ErrorKind {
    fn from(e: tokio::sync::broadcast::error::SendError<ToMinionMessage>) -> ErrorKind {
        ErrorKind::BroadcastSend(format!("{}", e))
    }
}

impl From<tokio::sync::broadcast::error::RecvError> for ErrorKind {
    fn from(e: tokio::sync::broadcast::error::RecvError) -> ErrorKind {
        ErrorKind::BroadcastReceive(e)
    }
}

impl From<String> for ErrorKind {
    fn from(s: String) -> ErrorKind {
        ErrorKind::General(s)
    }
}

impl From<&str> for ErrorKind {
    fn from(s: &str) -> ErrorKind {
        ErrorKind::General(s.to_string())
    }
}

impl From<http::Error> for ErrorKind {
    fn from(e: http::Error) -> ErrorKind {
        ErrorKind::HttpError(e)
    }
}

impl From<tokio::task::JoinError> for ErrorKind {
    fn from(e: tokio::task::JoinError) -> ErrorKind {
        ErrorKind::JoinError(e)
    }
}

impl From<tokio::sync::mpsc::error::SendError<ToOverlordMessage>> for ErrorKind {
    fn from(e: tokio::sync::mpsc::error::SendError<ToOverlordMessage>) -> ErrorKind {
        ErrorKind::MpscSend(e)
    }
}

impl From<nostr_types::Error> for ErrorKind {
    fn from(e: nostr_types::Error) -> ErrorKind {
        ErrorKind::Nostr(e)
    }
}

impl From<image::error::ImageError> for ErrorKind {
    fn from(e: image::error::ImageError) -> ErrorKind {
        ErrorKind::Image(e)
    }
}

impl From<std::io::Error> for ErrorKind {
    fn from(e: std::io::Error) -> ErrorKind {
        ErrorKind::Io(e)
    }
}

impl From<http::uri::InvalidUriParts> for ErrorKind {
    fn from(e: http::uri::InvalidUriParts) -> ErrorKind {
        ErrorKind::InvalidUriParts(e)
    }
}

impl From<http::uri::InvalidUri> for ErrorKind {
    fn from(e: http::uri::InvalidUri) -> ErrorKind {
        ErrorKind::InvalidUri(e)
    }
}

impl From<heed::Error> for ErrorKind {
    fn from(e: heed::Error) -> ErrorKind {
        ErrorKind::Lmdb(e)
    }
}

impl From<std::num::ParseIntError> for ErrorKind {
    fn from(e: std::num::ParseIntError) -> ErrorKind {
        ErrorKind::ParseInt(e)
    }
}

impl From<std::str::ParseBoolError> for ErrorKind {
    fn from(e: std::str::ParseBoolError) -> Self {
        ErrorKind::ParseBool(e)
    }
}

impl From<regex::Error> for ErrorKind {
    fn from(e: regex::Error) -> ErrorKind {
        ErrorKind::Regex(e)
    }
}

impl From<reqwest::Error> for ErrorKind {
    fn from(e: reqwest::Error) -> ErrorKind {
        ErrorKind::ReqwestHttpError(e)
    }
}

impl From<serde_json::Error> for ErrorKind {
    fn from(e: serde_json::Error) -> ErrorKind {
        ErrorKind::SerdeJson(e)
    }
}

impl From<std::array::TryFromSliceError> for ErrorKind {
    fn from(e: std::array::TryFromSliceError) -> ErrorKind {
        ErrorKind::SliceError(e)
    }
}

impl From<speedy::Error> for ErrorKind {
    fn from(e: speedy::Error) -> ErrorKind {
        ErrorKind::Speedy(e)
    }
}

impl From<usvg::Error> for ErrorKind {
    fn from(e: usvg::Error) -> ErrorKind {
        ErrorKind::Svg(e)
    }
}

impl From<tokio::time::error::Elapsed> for ErrorKind {
    fn from(e: tokio::time::error::Elapsed) -> ErrorKind {
        ErrorKind::Timeout(e)
    }
}

impl From<std::str::Utf8Error> for ErrorKind {
    fn from(e: std::str::Utf8Error) -> ErrorKind {
        ErrorKind::Utf8Error(e)
    }
}

impl From<tungstenite::Error> for ErrorKind {
    fn from(e: tungstenite::Error) -> ErrorKind {
        ErrorKind::Websocket(e)
    }
}

impl From<url::ParseError> for ErrorKind {
    fn from(e: url::ParseError) -> ErrorKind {
        ErrorKind::UrlParse(e)
    }
}

impl From<std::string::FromUtf8Error> for ErrorKind {
    fn from(e: std::string::FromUtf8Error) -> ErrorKind {
        ErrorKind::FromUtf8(e)
    }
}
