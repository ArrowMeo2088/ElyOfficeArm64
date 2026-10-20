//! An account's servers and how it signs in (MAIL-01, 02).

use serde::{Deserialize, Serialize};

use crate::{Error, Imap, Login, Pop, Result, Secret, Server, Smtp, TlsConnector, oauth};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Imap,
    Pop,
}

/// A password, or OAuth with one provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Auth {
    Password,
    Google,
    Microsoft,
}

impl Auth {
    /// The OAuth server; None for a password.
    pub fn provider(self) -> Option<Result<oauth::Provider>> {
        match self {
            Auth::Password => None,
            Auth::Google => Some(oauth::google()),
            Auth::Microsoft => Some(oauth::microsoft()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub address: String,
    pub user: String,
    pub auth: Auth,
    pub protocol: Protocol,
    pub incoming: Server,
    pub outgoing: Server,
}

/// What each server answered.
pub struct Checked {
    pub incoming: Result<()>,
    pub outgoing: Result<()>,
}

impl Checked {
    pub fn passed(&self) -> bool {
        self.incoming.is_ok() && self.outgoing.is_ok()
    }
}

impl Account {
    /// The vault name of its password or refresh token.
    pub fn secret_name(&self) -> String {
        format!("account:{}", self.address)
    }

    /// Refuses what no server could carry.
    pub fn valid(&self) -> Result<()> {
        crate::smtp::mailbox(&self.address)?;
        if self.user.is_empty() || self.user.contains(['\r', '\n']) {
            return Err(Error::Invalid("a user name on one line is needed".into()));
        }
        for server in [&self.incoming, &self.outgoing] {
            if server.host.is_empty() || server.host.contains(char::is_whitespace) {
                return Err(Error::Invalid(format!("a server host: {:?}", server.host)));
            }
            if server.port == 0 {
                return Err(Error::Invalid("port 0".into()));
            }
        }
        Ok(())
    }

    pub fn login(&self, secret: Secret) -> Login {
        Login {
            user: self.user.clone(),
            secret,
        }
    }

    /// Logs in to both servers at once, then leaves.
    pub async fn check(&self, secret: Secret, tls: &TlsConnector) -> Checked {
        let login = self.login(secret);
        let (incoming, outgoing) = tokio::join!(
            self.check_incoming(&login, tls),
            check_outgoing(&self.outgoing, &login, tls)
        );
        log::info!(
            "mail: check: incoming {}, outgoing {}",
            outcome(&incoming),
            outcome(&outgoing)
        );
        Checked { incoming, outgoing }
    }

    async fn check_incoming(&self, login: &Login, tls: &TlsConnector) -> Result<()> {
        match self.protocol {
            Protocol::Imap => {
                Imap::connect(&self.incoming, login, tls)
                    .await?
                    .logout()
                    .await
            }
            Protocol::Pop => Pop::connect(&self.incoming, login, tls).await?.quit().await,
        }
    }
}

async fn check_outgoing(server: &Server, login: &Login, tls: &TlsConnector) -> Result<()> {
    Smtp::connect(server, login, tls).await?.quit().await
}

fn outcome(result: &Result<()>) -> &'static str {
    match result {
        Ok(()) => "passed",
        Err(_) => "failed",
    }
}
