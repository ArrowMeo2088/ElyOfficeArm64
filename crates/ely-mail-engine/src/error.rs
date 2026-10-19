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
        }
    }
}

impl std::error::Error for Error {}
