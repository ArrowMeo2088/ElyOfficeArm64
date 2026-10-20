//! IMAP accounts sync on open and once added.

use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use ely_mail_engine::{AccountId, Protocol, Synced, sync};
use gpui::Context;
use tokio::task::AbortHandle;

use crate::mail::Mail;

/// One account's sync as the window shows it.
#[derive(Default)]
pub struct Syncing {
    pub task: Option<AbortHandle>,
    pub synced: Option<Synced>,
    pub failed: Option<String>,
}

pub type Syncs = HashMap<AccountId, Syncing>;

impl Mail {
    pub fn sync(&mut self, id: AccountId, cx: &mut Context<Self>) {
        let Some((_, account)) = self.accounts().iter().find(|(each, _)| *each == id) else {
            return log::info!("mail: sync asked of a gone account");
        };
        if account.protocol != Protocol::Imap {
            return;
        }
        if self.syncs.get(&id).is_some_and(|each| each.task.is_some()) {
            return log::info!("mail: account {} is already syncing", id.0);
        }
        let Ok(store) = self.store.clone() else {
            return log::error!("mail: no store to sync with");
        };
        let tls = match self.tls.clone() {
            Ok(tls) => tls,
            Err(error) => {
                log::warn!("mail: account {} cannot sync without TLS", id.0);
                self.syncs.entry(id).or_default().failed = Some(error);
                return self.read_synced(id, cx);
            }
        };
        let (account, vault) = (account.clone(), self.vault.clone());
        let work = async move {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|err| ely_mail_engine::Error::Store(format!("the clock: {err}")))?
                .as_secs() as i64;
            let secret = sync::secret(&account, &vault).await?;
            sync::imap(&account, id, secret, &tls, &store, now).await
        };
        let task = self.on_runtime(cx, work, move |mail, done, cx| {
            let failed = match done {
                Ok(Ok(_)) => None,
                Ok(Err(error)) => Some(error.to_string()),
                Err(error) => Some(error),
            };
            if failed.is_some() {
                log::warn!("mail: account {} did not sync", id.0);
            }
            let Some(syncing) = mail.syncs.get_mut(&id) else {
                return log::info!("mail: a sync ended for a removed account");
            };
            syncing.task = None;
            syncing.failed = failed;
            mail.read_synced(id, cx);
        });
        self.syncs.entry(id).or_default().task = Some(task);
        self.read_synced(id, cx);
    }

    /// Stops the account's sync and forgets it.
    pub fn stop_sync(&mut self, id: AccountId) {
        if let Some(task) = self.syncs.remove(&id).and_then(|each| each.task) {
            task.abort();
        }
    }

    fn read_synced(&mut self, id: AccountId, cx: &mut Context<Self>) {
        self.with_store(
            cx,
            move |store| store.synced(id),
            move |mail, read, cx| match (read, mail.syncs.get_mut(&id)) {
                (Ok(synced), Some(syncing)) => syncing.synced = Some(synced),
                (Err(error), Some(_)) => mail.failed("mail.sync.unread", &error, cx),
                (_, None) => {}
            },
        );
    }
}
