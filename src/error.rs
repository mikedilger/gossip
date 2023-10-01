use crate::comms::{ToMinionMessage, ToOverlordMessage};

#[derive(Debug)]
pub enum ErrorKind {
    BroadcastSend(tokio::sync::broadcast::error::SendError<ToMinionMessage>),
    BroadcastReceive(tokio::sync::broadcast::error::RecvError),
    Delegation(String),
    Empty(String),
    EventNotFound,
    General(String),
    GroupDmsNotYetSupported,
    HttpError(http::Error),
    JoinError(tokio::task::JoinError),
    Lmdb(heed::Error),
    MaxRelaysReached,
    MpscSend(tokio::sync::mpsc::error::SendError<ToOverlordMessage>),
    Nip05KeyNotFound,
    Nostr(nostr_types::Error),
    NoPrivateKey,
    NoRelay,
    Image(image::error::ImageError),
    Io(std::io::Error),
    Internal(String),
    InvalidUriParts(http::uri::InvalidUriParts),
    InvalidDnsId,
    InvalidUri(http::uri::InvalidUri),
    InvalidUrl(String),
    ParseInt(std::num::ParseIntError),
    Regex(regex::Error),
    RelayPickerError(gossip_relay_picker::Error),
    RelayRejectedUs,
    ReqwestHttpError(reqwest::Error),
    Sql(rusqlite::Error),
    SerdeJson(serde_json::Error),
    SliceError(std::array::TryFromSliceError),
    Speedy(speedy::Error),
    Timeout(tokio::time::error::Elapsed),
    UnknownCommand(String),
    UrlHasEmptyHostname,
    UrlHasNoHostname,
    UrlParse(url::ParseError),
    Usage(String, String), // error, usage line
    Utf8Error(std::str::Utf8Error),
    Websocket(tungstenite::Error),
    WrongEventKind,
}

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
            BroadcastSend(e) => write!(f, "Error broadcasting: {e}"),
            BroadcastReceive(e) => write!(f, "Error receiving broadcast: {e}"),
            Delegation(s) => write!(f, "NIP-26 Delegation Error: {s}"),
            Empty(s) => write!(f, "{s} is empty"),
            EventNotFound => write!(f, "Event not found"),
            GroupDmsNotYetSupported => write!(f, "Group DMs are not yet supported"),
            General(s) => write!(f, "{s}"),
            HttpError(e) => write!(f, "HTTP error: {e}"),
            JoinError(e) => write!(f, "Task join error: {e}"),
            Lmdb(e) => write!(f, "LMDB: {e}"),
            MaxRelaysReached => write!(
                f,
                "Maximum relay connections reached, will not connect to another"
            ),
            MpscSend(e) => write!(f, "Error sending mpsc: {e}"),
            Nip05KeyNotFound => write!(f, "NIP-05 public key not found"),
            Nostr(e) => write!(f, "Nostr: {e}"),
            NoPrivateKey => write!(f, "No private key available."),
            NoRelay => write!(f, "Could not determine a relay to use."),
            Image(e) => write!(f, "Image: {e}"),
            Io(e) => write!(f, "I/O Error: {e}"),
            Internal(s) => write!(f, "INTERNAL: {s}"),
            InvalidUriParts(e) => write!(f, "Invalid URI parts: {e}"),
            InvalidDnsId => write!(f, "Invalid DNS ID (nip-05), should be user@domain"),
            InvalidUri(e) => write!(f, "Invalid URI: {e}"),
            InvalidUrl(s) => write!(f, "Invalid URL: {s}"),
            ParseInt(e) => write!(f, "Bad integer: {e}"),
            Regex(e) => write!(f, "Regex: {e}"),
            RelayPickerError(e) => write!(f, "Relay Picker error: {e}"),
            RelayRejectedUs => write!(f, "Relay rejected us."),
            ReqwestHttpError(e) => write!(f, "HTTP (reqwest) error: {e}"),
            Sql(e) => write!(f, "SQL: {e}"),
            SerdeJson(e) => write!(f, "SerdeJson Error: {e}"),
            SliceError(e) => write!(f, "Slice: {e}"),
            Speedy(e) => write!(f, "Speedy: {e}"),
            Timeout(e) => write!(f, "Timeout: {e}"),
            UnknownCommand(s) => write!(f, "Unknown command: {s}"),
            UrlHasEmptyHostname => write!(f, "URL has empty hostname"),
            UrlHasNoHostname => write!(f, "URL has no hostname"),
            UrlParse(e) => write!(f, "URL parse: {e}"),
            Usage(e, u) => write!(f, "{}\n\nUsage: {}", e, u),
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
        ErrorKind::BroadcastSend(e)
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

impl From<regex::Error> for ErrorKind {
    fn from(e: regex::Error) -> ErrorKind {
        ErrorKind::Regex(e)
    }
}

impl From<gossip_relay_picker::Error> for ErrorKind {
    fn from(e: gossip_relay_picker::Error) -> ErrorKind {
        ErrorKind::RelayPickerError(e)
    }
}

impl From<rusqlite::Error> for ErrorKind {
    fn from(e: rusqlite::Error) -> ErrorKind {
        ErrorKind::Sql(e)
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
