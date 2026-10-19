use std::{fmt, io};

pub type Result<T> = std::result::Result<T, Error>;

/// Why a mail server conversation failed; server words kept.
#[derive(Debug)]
pub enum Error {
    /// No TCP connection: name lookup, refusal, reset.
    Connect(io::Error),
    /// TLS handshake or certificate check failed.
    Tls(String),
    /// The server refused the credentials.
    Rejected(String),
    /// The server refused a command after login.
    Server(String),
    /// The server said something the client cannot read.
    Protocol(String),
    /// The server did not answer in time.
    Timeout(&'static str),
    /// An address or login the protocol cannot carry.
    Invalid(String),
    /// A message that cannot be read or written.
    Mime(String),
    /// The local store failed: database, disk or damage.
    Store(String),
    /// The store refuses writes; the reason says why.
    ReadOnly(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect(err) => write!(f, "cannot connect: {err}"),
            Self::Tls(text) => write!(f, "TLS failed: {text}"),
            Self::Rejected(text) => write!(f, "login refused: {text}"),
            Self::Server(text) => write!(f, "server refused: {text}"),
            Self::Protocol(text) => write!(f, "unreadable reply: {text}"),
            Self::Timeout(step) => write!(f, "no answer while {step}"),
            Self::Invalid(text) => write!(f, "cannot be sent: {text}"),
            Self::Mime(text) => write!(f, "unreadable message: {text}"),
            Self::Store(text) => write!(f, "local store failed: {text}"),
            Self::ReadOnly(text) => write!(f, "local store is read-only: {text}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Self::Store(err.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Store(format!("a stored field: {err}"))
    }
}
