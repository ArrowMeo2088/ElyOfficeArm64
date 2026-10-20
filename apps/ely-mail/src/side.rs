//! Every inbox, then each account's folders, down the left.

use ely_gpui_component::{
    layout::ScrollArea,
    mail::{Mailbox, MailboxList},
    primitives::IconName,
};
use ely_mail_engine::{LocalFolder, Role};
use gpui::{
    App, Context, IntoElement, ParentElement as _, Render, SharedString, Styled as _, WeakEntity,
    Window, div,
};

use crate::{
    boxes::Open,
    mail::{Mail, text},
};

pub struct Side {
    mail: WeakEntity<Mail>,
}

impl Side {
    pub fn new(mail: WeakEntity<Mail>) -> Self {
        Self { mail }
    }
}

impl Render for Side {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(mail) = self.mail.upgrade() else {
            return div().into_any_element();
        };
        let mail = mail.read(cx);
        let inboxes = Mailbox::new(
            Open::Inboxes.key(),
            text(cx, "mail.boxes.all", &[]),
            IconName::Inbox,
        )
        .unread(mail.inboxes_unread());
        let drafts = Mailbox::new(
            Open::Drafts.key(),
            text(cx, "mail.boxes.drafts", &[]),
            IconName::File,
        );
        let mut list = MailboxList::new("mail-boxes")
            .section(text(cx, "mail.boxes.unified", &[]), [inboxes, drafts]);
        for (id, account) in mail.accounts() {
            let folders = mail.syncs.get(id).map_or(&[][..], |each| &each.folders);
            let mut sorted: Vec<&LocalFolder> = folders.iter().collect();
            sorted.sort_by(|a, b| rank(a.role).cmp(&rank(b.role)).then(a.name.cmp(&b.name)));
            let boxes = sorted.into_iter().map(|folder| {
                let open = Open::Mailbox(*id, folder.id);
                // Boxes with nothing unread are absent.
                let unread = mail.unread.get(&folder.id).copied().unwrap_or(0);
                Mailbox::new(open.key(), name(folder, cx), icon(folder.role)).unread(unread)
            });
            let settings = Mailbox::new(
                Open::Account(*id).key(),
                text(cx, "mail.boxes.settings", &[]),
                IconName::Settings,
            );
            list = list.section(account.address.clone(), boxes.chain([settings]));
        }
        let this = self.mail.clone();
        let list = list.open(mail.open.key()).on_open(move |key, _, cx| {
            this.update(cx, |mail, cx| match mail.opened(key) {
                Some(open) => mail.open_box(open, cx),
                None => log::error!("mail: the mailbox list sent an unknown box"),
            })
            .ok();
        });
        ScrollArea::new("mail-side-scroll")
            .vertical()
            .size_full()
            .child(div().p_2().child(list))
            .into_any_element()
    }
}

impl Mail {
    /// The box a side key names, if still here.
    fn opened(&self, key: &SharedString) -> Option<Open> {
        for open in [Open::Inboxes, Open::Drafts] {
            if *key == open.key() {
                return Some(open);
            }
        }
        self.accounts().iter().find_map(|(id, _)| {
            let folders = self.syncs.get(id).map_or(&[][..], |each| &each.folders);
            let boxes = folders.iter().map(|folder| Open::Mailbox(*id, folder.id));
            boxes
                .chain([Open::Account(*id)])
                .find(|open| open.key() == *key)
        })
    }
}

/// Inbox in the reader's words; others as named.
fn name(folder: &LocalFolder, cx: &App) -> SharedString {
    match folder.role {
        Some(Role::Inbox) => text(cx, "mail.boxes.inbox", &[]),
        _ => folder.name.clone().into(),
    }
}

fn rank(role: Option<Role>) -> u8 {
    match role {
        Some(Role::Inbox) => 0,
        Some(Role::Flagged) => 1,
        Some(Role::Drafts) => 2,
        Some(Role::Sent) => 3,
        Some(Role::Archive) => 4,
        Some(Role::All) => 5,
        Some(Role::Junk) => 6,
        Some(Role::Trash) => 7,
        None => 8,
    }
}

fn icon(role: Option<Role>) -> IconName {
    match role {
        Some(Role::Inbox) => IconName::Inbox,
        Some(Role::Flagged) => IconName::Star,
        Some(Role::Drafts) => IconName::File,
        Some(Role::Sent) => IconName::Send,
        Some(Role::Archive) => IconName::Archive,
        Some(Role::All) => IconName::Mail,
        Some(Role::Junk) => IconName::TriangleAlert,
        Some(Role::Trash) => IconName::Trash2,
        None => IconName::Folder,
    }
}
