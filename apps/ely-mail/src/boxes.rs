//! What the window has open, and its list.

use std::{collections::HashMap, rc::Rc};

use ely_gpui_component::mail::Mail as Row;
use ely_mail_engine::{AccountId, Filter, MailboxId, Role, Scope, Summary};
use gpui::{Context, SharedString};

use crate::{
    draft_list,
    mail::{Mail, text},
};

pub type Unread = HashMap<MailboxId, u32>;

/// What the middle of the window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Open {
    Inboxes,
    /// Drafts kept here, of every account.
    Drafts,
    Mailbox(AccountId, MailboxId),
    /// The account's settings and sync.
    Account(AccountId),
}

impl Open {
    pub fn key(self) -> SharedString {
        match self {
            Open::Inboxes => "inboxes".into(),
            Open::Drafts => "drafts".into(),
            Open::Mailbox(_, mailbox) => format!("box-{}", mailbox.0).into(),
            Open::Account(id) => format!("account-{}", id.0).into(),
        }
    }

    pub fn account(self) -> Option<AccountId> {
        match self {
            Open::Inboxes | Open::Drafts => None,
            Open::Mailbox(id, _) | Open::Account(id) => Some(id),
        }
    }

    fn scope(self) -> Option<Scope> {
        match self {
            Open::Inboxes => Some(Scope::Inboxes),
            Open::Mailbox(_, mailbox) => Some(Scope::Mailbox(mailbox)),
            Open::Drafts | Open::Account(_) => None,
        }
    }
}

/// Rows read, and where each key sits.
pub struct Indexed {
    rows: Vec<Row>,
    at: HashMap<SharedString, usize>,
}

/// Off the UI thread: a long list takes a while.
pub fn index(rows: Vec<Row>) -> Indexed {
    let at = rows
        .iter()
        .enumerate()
        .map(|(ix, row)| (row.key.clone(), ix))
        .collect();
    Indexed { rows, at }
}

/// The open list and the messages picked in it.
#[derive(Default)]
pub struct Listing {
    /// Numbers reads; an older one is dropped.
    pub read: u64,
    /// None until read, or when nothing lists mail.
    pub mails: Option<Result<Rc<Vec<Row>>, String>>,
    at: HashMap<SharedString, usize>,
    pub selected: Vec<SharedString>,
    /// The one message picked, for the reading pane.
    pub chosen: Option<Row>,
}

impl Listing {
    pub fn show(&mut self, mails: Option<Result<Indexed, String>>) {
        self.at = HashMap::new();
        self.mails = mails.map(|read| {
            read.map(|indexed| {
                self.at = indexed.at;
                Rc::new(indexed.rows)
            })
        });
    }

    /// Keeps the picks still listed.
    fn pick(&mut self, keys: Vec<SharedString>) {
        let Some(Ok(mails)) = &self.mails else {
            self.selected.clear();
            self.chosen = None;
            return;
        };
        self.selected = keys
            .into_iter()
            .filter(|key| self.at.contains_key(key))
            .collect();
        self.chosen = match &self.selected[..] {
            [key] => Some(mails[self.at[key]].clone()),
            _ => None,
        };
    }
}

impl Mail {
    pub fn open_box(&mut self, open: Open, cx: &mut Context<Self>) {
        log::info!("mail: open {}", open.key());
        if open != self.open {
            self.open = open;
            if let Open::Account(id) = open {
                self.read_identity(id, cx);
                self.read_drafts_up(id, cx);
            }
            self.listing.show(None);
            self.listing.pick(Vec::new());
            self.read_letter(cx);
        }
        self.read_mail(cx);
        self.notify(cx);
    }

    pub fn set_filter(&mut self, filter: Filter, cx: &mut Context<Self>) {
        log::info!("mail: filter {filter:?}");
        self.filter = filter;
        self.read_mail(cx);
        cx.notify();
    }

    pub fn select(&mut self, keys: &[SharedString], cx: &mut Context<Self>) {
        self.listing.pick(keys.to_vec());
        self.read_letter(cx);
        cx.notify();
    }

    /// Unread counts, and the open list when it lists mail.
    pub fn read_mail(&mut self, cx: &mut Context<Self>) {
        self.listing.read += 1;
        let (read, open, filter) = (self.listing.read, self.open, self.filter);
        let scope = open.scope();
        let none = (
            text(cx, "mail.no_sender", &[]),
            text(cx, "mail.no_subject", &[]),
        );
        self.with_store(
            cx,
            move |store| {
                let mut kept = Vec::new();
                let rows = match scope {
                    Some(scope) => {
                        let list = store.list(scope, filter)?;
                        Some(index(
                            list.into_iter().map(|each| row(each, &none)).collect(),
                        ))
                    }
                    None if open == Open::Drafts => {
                        kept = store.drafts()?;
                        Some(index(draft_list::rows(&kept, &none)))
                    }
                    None => None,
                };
                Ok((rows, store.unread()?, kept))
            },
            move |mail, read_now, cx| {
                if mail.listing.read != read {
                    return log::info!("mail: list read {read} ended after a newer one");
                }
                match read_now {
                    Ok((rows, unread, kept)) => {
                        mail.unread = unread;
                        mail.shelved = kept;
                        mail.listing.show(rows.map(Ok));
                    }
                    Err(error) => {
                        log::warn!("mail: list read {read} failed: {error}");
                        let listed = scope.is_some() || open == Open::Drafts;
                        mail.listing.show(listed.then_some(Err(error)));
                    }
                }
                let keys = std::mem::take(&mut mail.listing.selected);
                mail.listing.pick(keys);
                mail.read_letter(cx);
            },
        );
    }

    /// Unread mail in every account's inbox.
    pub fn inboxes_unread(&self) -> u32 {
        self.syncs
            .values()
            .flat_map(|syncing| &syncing.folders)
            .filter(|folder| folder.role == Some(Role::Inbox))
            .filter_map(|folder| self.unread.get(&folder.id))
            .sum()
    }
}

/// `none` names a missing sender, then subject.
fn row(summary: Summary, none: &(SharedString, SharedString)) -> Row {
    let key = summary.id.0.to_string();
    let from = summary.from.first().map(|from| match &from.name {
        Some(name) if !name.trim().is_empty() => name.clone(),
        _ => from.email.clone(),
    });
    let from: SharedString = from.map_or_else(|| none.0.clone(), Into::into);
    let subject: SharedString = summary.subject.map_or_else(|| none.1.clone(), Into::into);
    let row = match summary.date.map(jiff::Timestamp::from_second) {
        Some(Ok(at)) => Row::new(key, from, subject, at),
        Some(Err(error)) => {
            log::warn!("mail: message {} date unshown: {error}", summary.id.0);
            Row::undated(key, from, subject)
        }
        None => Row::undated(key, from, subject),
    };
    let row = row.unread(!summary.seen).starred(summary.flagged);
    match summary.attachments {
        Some(count) => row.attachments(count as usize),
        None => row,
    }
}
