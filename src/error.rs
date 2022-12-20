use crate::comms::BusMessage;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Error broadcasting: {0}")]
    BroadcastSend(#[from] tokio::sync::broadcast::error::SendError<BusMessage>),

    #[error("Error receiving broadcast: {0}")]
    BroadcastReceive(#[from] tokio::sync::broadcast::error::RecvError),

    #[error("Error: {0}")]
    General(String),

    #[error("Task join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("Error sending mpsc: {0}")]
    MpscSend(#[from] tokio::sync::mpsc::error::SendError<BusMessage>),

    #[error("Nostr: {0}")]
    Nostr(#[from] nostr_proto::Error),

    #[error("Image: {0}")]
    Image(#[from] image::error::ImageError),

    #[error("I/O Error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Bad integer: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    #[error("SerdeJson Error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("SQL: {0}")]
    Sql(#[from] rusqlite::Error),
}

impl From<String> for Error {
    fn from(s: String) -> Error {
        Error::General(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Error {
        Error::General(s.to_string())
    }
}
