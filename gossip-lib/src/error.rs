use crate::comms::{ToMinionMessage, ToOverlordMessage};
use crate::people::PersonList;
use nostr_types::RelayUrl;
use std::panic::Location;

/// Error kinds that can occur in gossip-lib
#[derive(Debug)]
pub enum ErrorKind {
    BadNostrConnectString,
    BlossomError(String),
    BroadcastSend(String),
    BroadcastReceive(tokio::sync::broadcast::error::RecvError),
    CannotUpdateRelayUrl,
    Delegation(String),
    Disconnected,
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
    KeyNotExportable,
    KeySizeWrong,
    KeyInvalid,
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
    TimedOut,
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
    location: &'static Location<'static>,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}, {}", self.kind, self.location)
    }
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ErrorKind::*;
        match self {
            BadNostrConnectString => write!(f, "Bad nostrconnect string"),
            BlossomError(s) => write!(f, "Blossom error: {s}"),
            BroadcastSend(s) => write!(f, "Error broadcasting: {s}"),
            BroadcastReceive(e) => write!(f, "Error receiving broadcast: {e}"),
            CannotUpdateRelayUrl => {
                write!(f, "Cannot update relay url (create a new relay instead)")
            }
            Delegation(s) => write!(f, "NIP-26 Delegation Error: {s}"),
            Disconnected => write!(f, "Disconnected"),
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
            KeyNotExportable => write!(f, "Key not exportable"),
            KeySizeWrong => write!(f, "Key size is wrong"),
            KeyInvalid => write!(f, "Key is invalid"),
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
            TimedOut => write!(f, "Timed out"),
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

// Note: we impl Into because our typical pattern is InnerError::Variant.into()
//       when we tried implementing From, the location was deep in rust code's
//       blanket into implementation, which wasn't the line number we wanted.
//
//       As for converting other error types, the try! macro uses From so it
//       is correct.
#[allow(clippy::from_over_into)]
impl Into<Error> for ErrorKind {
    #[track_caller]
    fn into(self) -> Error {
        Error {
            kind: self,
            location: Location::caller(),
        }
    }
}

impl From<tokio::sync::broadcast::error::SendError<ToMinionMessage>> for Error {
    #[track_caller]
    fn from(e: tokio::sync::broadcast::error::SendError<ToMinionMessage>) -> Error {
        Error {
            kind: ErrorKind::BroadcastSend(format!("{}", e)),
            location: Location::caller(),
        }
    }
}

impl From<tokio::sync::broadcast::error::RecvError> for Error {
    #[track_caller]
    fn from(e: tokio::sync::broadcast::error::RecvError) -> Error {
        Error {
            kind: ErrorKind::BroadcastReceive(e),
            location: Location::caller(),
        }
    }
}

impl From<String> for Error {
    #[track_caller]
    fn from(s: String) -> Error {
        Error {
            kind: ErrorKind::General(s),
            location: Location::caller(),
        }
    }
}

impl From<&str> for Error {
    #[track_caller]
    fn from(s: &str) -> Error {
        Error {
            kind: ErrorKind::General(s.to_string()),
            location: Location::caller(),
        }
    }
}

impl From<http::Error> for Error {
    #[track_caller]
    fn from(e: http::Error) -> Error {
        Error {
            kind: ErrorKind::HttpError(e),
            location: Location::caller(),
        }
    }
}

impl From<tokio::task::JoinError> for Error {
    #[track_caller]
    fn from(e: tokio::task::JoinError) -> Error {
        Error {
            kind: ErrorKind::JoinError(e),
            location: Location::caller(),
        }
    }
}

impl From<tokio::sync::mpsc::error::SendError<ToOverlordMessage>> for Error {
    #[track_caller]
    fn from(e: tokio::sync::mpsc::error::SendError<ToOverlordMessage>) -> Error {
        Error {
            kind: ErrorKind::MpscSend(e),
            location: Location::caller(),
        }
    }
}

impl From<nostr_types::Error> for Error {
    #[track_caller]
    fn from(e: nostr_types::Error) -> Error {
        Error {
            kind: ErrorKind::Nostr(e),
            location: Location::caller(),
        }
    }
}

impl From<image::error::ImageError> for Error {
    #[track_caller]
    fn from(e: image::error::ImageError) -> Error {
        Error {
            kind: ErrorKind::Image(e),
            location: Location::caller(),
        }
    }
}

impl From<std::io::Error> for Error {
    #[track_caller]
    fn from(e: std::io::Error) -> Error {
        Error {
            kind: ErrorKind::Io(e),
            location: Location::caller(),
        }
    }
}

impl From<http::uri::InvalidUriParts> for Error {
    #[track_caller]
    fn from(e: http::uri::InvalidUriParts) -> Error {
        Error {
            kind: ErrorKind::InvalidUriParts(e),
            location: Location::caller(),
        }
    }
}

impl From<http::uri::InvalidUri> for Error {
    #[track_caller]
    fn from(e: http::uri::InvalidUri) -> Error {
        Error {
            kind: ErrorKind::InvalidUri(e),
            location: Location::caller(),
        }
    }
}

impl From<heed::Error> for Error {
    #[track_caller]
    fn from(e: heed::Error) -> Error {
        Error {
            kind: ErrorKind::Lmdb(e),
            location: Location::caller(),
        }
    }
}

impl From<std::num::ParseIntError> for Error {
    #[track_caller]
    fn from(e: std::num::ParseIntError) -> Error {
        Error {
            kind: ErrorKind::ParseInt(e),
            location: Location::caller(),
        }
    }
}

impl From<std::str::ParseBoolError> for Error {
    #[track_caller]
    fn from(e: std::str::ParseBoolError) -> Self {
        Error {
            kind: ErrorKind::ParseBool(e),
            location: Location::caller(),
        }
    }
}

impl From<regex::Error> for Error {
    #[track_caller]
    fn from(e: regex::Error) -> Error {
        Error {
            kind: ErrorKind::Regex(e),
            location: Location::caller(),
        }
    }
}

impl From<reqwest::Error> for Error {
    #[track_caller]
    fn from(e: reqwest::Error) -> Error {
        Error {
            kind: ErrorKind::ReqwestHttpError(e),
            location: Location::caller(),
        }
    }
}

impl From<serde_json::Error> for Error {
    #[track_caller]
    fn from(e: serde_json::Error) -> Error {
        Error {
            kind: ErrorKind::SerdeJson(e),
            location: Location::caller(),
        }
    }
}

impl From<std::array::TryFromSliceError> for Error {
    #[track_caller]
    fn from(e: std::array::TryFromSliceError) -> Error {
        Error {
            kind: ErrorKind::SliceError(e),
            location: Location::caller(),
        }
    }
}

impl From<speedy::Error> for Error {
    #[track_caller]
    fn from(e: speedy::Error) -> Error {
        Error {
            kind: ErrorKind::Speedy(e),
            location: Location::caller(),
        }
    }
}

impl From<usvg::Error> for Error {
    #[track_caller]
    fn from(e: usvg::Error) -> Error {
        Error {
            kind: ErrorKind::Svg(e),
            location: Location::caller(),
        }
    }
}

impl From<tokio::time::error::Elapsed> for Error {
    #[track_caller]
    fn from(e: tokio::time::error::Elapsed) -> Error {
        Error {
            kind: ErrorKind::Timeout(e),
            location: Location::caller(),
        }
    }
}

impl From<std::str::Utf8Error> for Error {
    #[track_caller]
    fn from(e: std::str::Utf8Error) -> Error {
        Error {
            kind: ErrorKind::Utf8Error(e),
            location: Location::caller(),
        }
    }
}

impl From<tungstenite::Error> for Error {
    #[track_caller]
    fn from(e: tungstenite::Error) -> Error {
        Error {
            kind: ErrorKind::Websocket(e),
            location: Location::caller(),
        }
    }
}

impl From<url::ParseError> for Error {
    #[track_caller]
    fn from(e: url::ParseError) -> Error {
        Error {
            kind: ErrorKind::UrlParse(e),
            location: Location::caller(),
        }
    }
}

impl From<std::string::FromUtf8Error> for Error {
    #[track_caller]
    fn from(e: std::string::FromUtf8Error) -> Error {
        Error {
            kind: ErrorKind::FromUtf8(e),
            location: Location::caller(),
        }
    }
}
