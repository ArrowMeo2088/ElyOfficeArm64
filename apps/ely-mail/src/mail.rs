//! The window: accounts and the dialogs that change them.

use std::{
    future::Future,
    path::Path,
    sync::{Arc, Mutex},
};

use ely_gpui_component::primitives::Severity;
use ely_mail_engine::{Account, AccountId, Filter, Store, TlsConnector, Vault};
use ely_platform::AppId;
use ely_ui::Root;
use gpui::{App, AppContext as _, Context, Entity, FocusHandle, SharedString, WeakEntity, Window};
use tokio::runtime::Runtime;

use crate::{
    adding::Adding,
    boxes::{Listing, Open, Unread},
    reading::Reading,
    removing::Removing,
    side::Side,
    syncing::Syncs,
};

/// What the engine runs on and keeps to.
pub struct Parts {
    pub store: Result<Store, String>,
    pub vault: Vault,
    pub tls: Result<TlsConnector, String>,
}

impl Parts {
    pub fn system(dir: &Path) -> Self {
        Self {
            store: Store::open(dir).map_err(|err| err.to_string()),
            vault: Vault::system(),
            tls: ely_mail_engine::system_tls().map_err(|err| err.to_string()),
        }
    }
}

pub struct Mail {
    pub store: Result<Arc<Mutex<Store>>, String>,
    pub read_only: Option<String>,
    pub vault: Arc<Vault>,
    pub tls: Result<TlsConnector, String>,
    runtime: Arc<Runtime>,
    /// None until read.
    pub accounts: Option<Result<Vec<(AccountId, Account)>, String>>,
    /// Remove acts on its account.
    pub open: Open,
    pub filter: Filter,
    pub listing: Listing,
    pub reading: Reading,
    pub unread: Unread,
    pub adding: Option<Adding>,
    pub removing: Option<Removing>,
    pub syncs: Syncs,
    /// Numbers every sync, even across forgotten accounts.
    pub runs: u64,
    /// Until resumed, no account syncs.
    pub paused: bool,
    /// Numbers every request across dialogs.
    pub asked: u64,
    pub focus: FocusHandle,
    pub root: WeakEntity<Root>,
    pub side: Entity<Side>,
}

pub fn text(cx: &App, key: &str, args: &[(&str, &str)]) -> SharedString {
    ely_ui::i18n::text(cx, key, args)
}

pub fn open(app: AppId, window: &mut Window, cx: &mut App) -> Entity<Root> {
    let dir = match ely_platform::data_dir(app) {
        Ok(dir) => dir,
        Err(error) => ely_platform::startup::fail(app, &format!("{error:#}")),
    };
    let parts = Parts::system(&dir);
    let mail = cx.new(|cx| Mail::new(parts, cx));
    let root = ely_ui::tool_root(app, mail.clone().into(), crate::menu::menus(cx), window, cx);
    mail.update(cx, |mail, cx| {
        mail.root = root.downgrade();
        window.focus(&mail.focus, cx);
    });
    root
}

impl Mail {
    pub fn new(parts: Parts, cx: &mut Context<Self>) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("mail")
            .enable_all()
            .build()
            .unwrap_or_else(|error| {
                ely_platform::startup::fail(AppId::Mail, &format!("no mail runtime: {error}"))
            });
        let read_only = match &parts.store {
            Ok(store) => store.read_only().map(str::to_owned),
            Err(error) => {
                log::error!("mail: the store did not open: {error}");
                None
            }
        };
        if let Err(error) = &parts.tls {
            log::error!("mail: no system TLS: {error}");
        }
        let this = cx.weak_entity();
        let mut mail = Mail {
            store: parts.store.map(|store| Arc::new(Mutex::new(store))),
            read_only,
            vault: Arc::new(parts.vault),
            tls: parts.tls,
            runtime: Arc::new(runtime),
            accounts: None,
            open: Open::Inboxes,
            filter: Filter::default(),
            listing: Listing::default(),
            reading: Reading::default(),
            unread: Unread::new(),
            adding: None,
            removing: None,
            syncs: Syncs::new(),
            runs: 0,
            paused: false,
            asked: 0,
            focus: cx.focus_handle(),
            root: WeakEntity::new_invalid(),
            side: cx.new(|_| Side::new(this)),
        };
        mail.read_accounts(cx);
        Self::sync_on_a_timer(cx);
        mail
    }

    pub fn read_accounts(&mut self, cx: &mut Context<Self>) {
        self.with_store(
            cx,
            |store| store.accounts(),
            |mail, read, cx| {
                let first = mail.accounts.is_none();
                if let Ok(accounts) = &read {
                    let still = |id| accounts.iter().any(|(each, _)| *each == id);
                    if mail.open.account().is_some_and(|id| !still(id)) {
                        mail.open = Open::Inboxes;
                    }
                }
                mail.accounts = Some(read);
                if first {
                    mail.sync_all(cx);
                }
                mail.read_mail(cx);
            },
        );
    }

    pub fn accounts(&self) -> &[(AccountId, Account)] {
        match &self.accounts {
            Some(Ok(accounts)) => accounts,
            _ => &[],
        }
    }

    /// Redraws the window and its account list.
    pub fn notify(&self, cx: &mut Context<Self>) {
        self.side.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    /// Store work off the window's thread.
    pub fn with_store<R: Send + 'static>(
        &self,
        cx: &mut Context<Self>,
        work: impl FnOnce(&mut Store) -> ely_mail_engine::Result<R> + Send + 'static,
        then: impl FnOnce(&mut Mail, Result<R, String>, &mut Context<Mail>) + 'static,
    ) {
        let store = match &self.store {
            Ok(store) => store.clone(),
            Err(error) => {
                self.failed("mail.error.store", error, cx);
                return;
            }
        };
        let read = cx.background_spawn(async move {
            let mut store = store
                .lock()
                .map_err(|_| "a store task panicked".to_owned())?;
            work(&mut store).map_err(|err| err.to_string())
        });
        cx.spawn(async move |this, cx| {
            let read = read.await;
            this.update(cx, |mail, cx| {
                then(mail, read, cx);
                mail.notify(cx);
            })
            .ok();
        })
        .detach();
    }

    /// Network work on the mail runtime; abort stops it.
    pub fn on_runtime<R: Send + 'static>(
        &self,
        cx: &mut Context<Self>,
        work: impl Future<Output = R> + Send + 'static,
        then: impl FnOnce(&mut Mail, Result<R, String>, &mut Context<Mail>) + 'static,
    ) -> tokio::task::AbortHandle {
        let task = self.runtime.spawn(work);
        let abort = task.abort_handle();
        cx.spawn(async move |this, cx| {
            let done = task.await;
            if done.as_ref().is_err_and(|err| err.is_cancelled()) {
                return;
            }
            let done = done.map_err(|err| format!("a mail task failed: {err}"));
            this.update(cx, |mail, cx| {
                then(mail, done, cx);
                mail.notify(cx);
            })
            .ok();
        })
        .detach();
        abort
    }

    /// Not logged: the words may name an address.
    pub fn toast(&self, text: SharedString, severity: Severity, cx: &mut App) {
        if let Some(root) = self.root.upgrade() {
            root.update(cx, |root, cx| root.toast(text, severity, cx));
        }
    }

    pub fn failed(&self, key: &str, error: &str, cx: &mut App) {
        log::warn!("mail: {key}");
        let words = text(cx, key, &[("error", error)]);
        self.toast(words, Severity::Danger, cx);
    }
}
