//! Mail protocols, sync and local storage for ElyMail.

mod error;
mod imap;
mod mime;
mod net;
mod pop;
mod smtp;
mod wire;

pub use error::{Error, Result};
pub use imap::Imap;
pub use mime::{Built, Draft, Mail, Mailbox, Part, build, read};
pub use net::{Login, Secret, Security, Server, system_tls};
pub use pop::Pop;
pub use smtp::Smtp;
pub use tokio_rustls::TlsConnector;
