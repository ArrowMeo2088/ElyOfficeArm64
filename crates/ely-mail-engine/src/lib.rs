//! Mail protocols, sync and local storage for ElyMail.

mod account;
mod error;
mod imap;
mod mime;
mod net;
pub mod oauth;
mod pop;
mod smtp;
mod store;
pub mod sync;
mod vault;
mod wire;

pub use account::{Account, Auth, Checked, Protocol};
pub use error::{Error, Result};
pub use imap::{Folder, Got, Imap, Opened, Role};
pub use mime::{Built, Draft, Mail, Mailbox, Part, build, read};
pub use net::{Login, Secret, Security, Server, system_tls};
pub use pop::Pop;
pub use smtp::Smtp;
pub use store::{
    AccountId, Cached, Fetched, Filter, Held, Key, LocalFolder, MailboxId, MessageId, Scope, Sizes,
    Store, Stored, Summary, Synced,
};
pub use tokio_rustls::TlsConnector;
pub use vault::{Backend, Kept, Keyring, Vault};
