//! Mail protocols, sync and local storage for ElyMail.

mod error;
mod imap;
mod mime;
mod net;
pub mod oauth;
mod pop;
mod smtp;
mod store;
mod vault;
mod wire;

pub use error::{Error, Result};
pub use imap::Imap;
pub use mime::{Built, Draft, Mail, Mailbox, Part, build, read};
pub use net::{Login, Secret, Security, Server, system_tls};
pub use pop::Pop;
pub use smtp::Smtp;
pub use store::{AccountId, Fetched, Key, MailboxId, MessageId, Sizes, Store, Stored};
pub use tokio_rustls::TlsConnector;
pub use vault::{Backend, Kept, Keyring, Vault};
