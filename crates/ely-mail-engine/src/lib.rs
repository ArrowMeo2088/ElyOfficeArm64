//! Mail protocols, sync and local storage for ElyMail.

mod account;
pub mod compose;
pub mod drafts;
mod error;
pub mod fetch;
mod html;
mod imap;
mod mime;
mod net;
pub mod oauth;
pub mod page;
mod pop;
mod smtp;
mod store;
pub mod sync;
mod vault;
mod wire;

pub use account::{Account, Auth, Checked, Protocol};
pub use error::{Error, Result};
pub use imap::{Folder, Got, Imap, Opened, Role};
pub use mime::{Built, Draft, Mail, Mailbox, Part, Shown, build, read};
pub use net::{Login, Secret, Security, Server, system_tls};
pub use pop::Pop;
pub use smtp::Smtp;
pub use store::{
    AccountId, Cached, DraftId, Fetched, Filter, Held, Identity, Key, Left, LocalFolder, MailboxId,
    MessageId, Remote, Scope, Shelved, Sizes, Store, Stored, Stowed, Summary, Synced, Unsent,
};
pub use tokio_rustls::TlsConnector;
pub use vault::{Backend, Kept, Keyring, Vault};
