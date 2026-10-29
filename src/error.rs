use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("time error: {0}")]
    Time(#[from] jiff::Error),

    #[error("usage: ote [<count>]")]
    Usage,

    #[error("DDG_TOKEN is not set")]
    MissingToken,

    #[error("bitwarden: {0}")]
    Bitwarden(String),
}

pub type Result<T> = std::result::Result<T, Error>;
