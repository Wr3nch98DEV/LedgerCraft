use std::fmt;

#[derive(Debug)]
pub enum Error {
    Db(rusqlite::Error),
    Json(serde_json::Error),
    Validation(String),
    NotFound(String),
    Locked(String),
    Forbidden(String),
    Io(std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Db(e) => {
                // Surface trigger messages (RAISE(ABORT, ...)) cleanly.
                if let rusqlite::Error::SqliteFailure(_, Some(m)) = e { write!(f, "{m}") } else { write!(f, "database error: {e}") }
            }
            Error::Json(e) => write!(f, "invalid data: {e}"),
            Error::Validation(m) => write!(f, "{m}"),
            Error::NotFound(m) => write!(f, "{m} not found"),
            Error::Locked(m) => write!(f, "{m}"),
            Error::Forbidden(m) => write!(f, "{m}"),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        if let rusqlite::Error::QueryReturnedNoRows = e { return Error::NotFound("record".into()); }
        Error::Db(e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self { Error::Json(e) }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self { Error::Io(e) }
}

pub fn invalid<T>(msg: impl Into<String>) -> Result<T> { Err(Error::Validation(msg.into())) }
