use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Error: {0}")]
    General(String),

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
