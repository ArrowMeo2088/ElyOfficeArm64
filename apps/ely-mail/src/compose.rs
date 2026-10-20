//! The compose window (C12.12, MAIL-05).

use std::path::PathBuf;

use ely_gpui_component::{
    documents::{MarkdownDocument, MarkdownMode},
    mail::Draft,
    primitives::Severity,
};
use ely_mail_engine::{AccountId, Error, Mailbox, Part, compose};
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, PathPromptOptions, SharedString,
    WeakEntity, Window,
};

mod page;

use crate::{
    mail::{Mail, text},
    view::local_time,
    writing::{Before, How, Opening, Outgoing, Sender, prepare},
};

/// An attached file and its key in the list.
pub struct File {
    pub key: u64,
    pub part: Part,
}

pub struct Compose {
    mail: WeakEntity<Mail>,
    pub senders: Vec<Sender>,
    pub from: AccountId,
    pub to: Vec<SharedString>,
    pub cc: Vec<SharedString>,
    pub subject: SharedString,
    in_reply_to: Option<String>,
    references: Vec<String>,
    pub before: Option<Before>,
    /// Only the answered message's headers are kept.
    pub headers_only: bool,
    pub markdown: bool,
    pub document: Entity<MarkdownDocument>,
    pub files: Vec<File>,
    next_file: u64,
    /// Files picked and not yet read.
    pub reading: usize,
    /// Forwarded files not downloaded.
    pub missing: usize,
    /// What it opened with, or last sent.
    base: Carried,
    /// The addresses and subject as now written.
    written: Draft,
    /// What the send under way carries.
    in_flight: Option<Carried>,
    pub sending: bool,
    pub problem: Option<(Severity, SharedString)>,
    pub discarding: bool,
    /// A quit waits on this message's answer.
    quitting: bool,
    focus: FocusHandle,
}

fn person(mailbox: &Mailbox) -> String {
    match &mailbox.name {
        Some(name) if !name.trim().is_empty() => format!("{name} <{}>", mailbox.email),
        _ => mailbox.email.clone(),
    }
}

fn people(list: &[Mailbox]) -> String {
    list.iter().map(person).collect::<Vec<_>>().join(", ")
}

/// A file's shown name; some have none.
pub fn file_name(part: &Part, cx: &App) -> SharedString {
    match &part.name {
        Some(name) => name.clone().into(),
        None => text(cx, "mail.write.unnamed", &[]),
    }
}

fn addresses(list: &[SharedString]) -> Vec<Mailbox> {
    list.iter()
        .map(|email| Mailbox {
            name: None,
            email: email.to_string(),
        })
        .collect()
}

impl Compose {
    pub fn new(
        mail: WeakEntity<Mail>,
        opening: Opening,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let document = cx.new(|cx| MarkdownDocument::new("", MarkdownMode::Visual, window, cx));
        let this = cx.weak_entity();
        ely_gpui_component::shell::guard_close(window, cx, move |window, cx| {
            this.update(cx, |compose, cx| compose.may_close(window, cx))
                .unwrap_or(true)
        });
        let mut compose = Compose {
            mail,
            senders: opening.senders,
            from: opening.from,
            to: Vec::new(),
            cc: Vec::new(),
            subject: SharedString::default(),
            in_reply_to: None,
            references: Vec::new(),
            before: None,
            headers_only: false,
            markdown: true,
            document,
            files: Vec::new(),
            next_file: 0,
            reading: 0,
            missing: 0,
            base: Carried {
                draft: blank(),
                words: String::new(),
                files: Vec::new(),
                from: opening.from,
                markdown: true,
            },
            written: blank(),
            in_flight: None,
            sending: false,
            problem: None,
            discarding: false,
            quitting: false,
            focus: cx.focus_handle(),
        };
        if let Some(answered) = opening.answered {
            let start = answered.start;
            compose.to = start.to.iter().map(|to| to.email.clone().into()).collect();
            compose.cc = start.cc.iter().map(|cc| cc.email.clone().into()).collect();
            compose.subject = start.subject.into();
            compose.in_reply_to = start.in_reply_to;
            compose.references = start.references;
            compose.headers_only = !answered.whole;
            compose.missing = answered.missing;
            for part in answered.files {
                compose.add_file(part);
            }
            if answered.whole {
                compose.before = Some(lead(answered.how, &answered.mail, answered.text, cx));
            }
        }
        compose.written = Draft {
            to: compose.to.clone(),
            cc: compose.cc.clone(),
            subject: compose.subject.trim().to_owned().into(),
            ..blank()
        };
        compose.base = compose.carried(cx);
        compose
    }

    /// The composer's addresses and subject after an edit.
    pub fn edited(&mut self, draft: Draft, cx: &mut Context<Self>) {
        self.written = draft;
        cx.notify();
    }

    fn sender(&self) -> Option<&Sender> {
        self.senders.iter().find(|sender| sender.id == self.from)
    }

    fn add_file(&mut self, part: Part) {
        self.next_file += 1;
        self.files.push(File {
            key: self.next_file,
            part,
        });
    }

    fn carried(&self, cx: &App) -> Carried {
        Carried {
            draft: self.written.clone(),
            words: self.document.read(cx).text(cx).trim().to_owned(),
            files: self.files.iter().map(|file| file.key).collect(),
            from: self.from,
            markdown: self.markdown,
        }
    }

    /// Anything written, taken off, or still being read.
    fn dirty(&self, cx: &App) -> bool {
        self.reading > 0 || self.carried(cx) != self.base
    }

    /// The window closes unless sending or holding words.
    pub fn may_close(&mut self, _: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.sending {
            self.problem = Some((Severity::Warning, text(cx, "mail.write.wait", &[])));
            cx.notify();
            return false;
        }
        if self.dirty(cx) {
            self.discarding = true;
            cx.notify();
            return false;
        }
        true
    }

    /// Lets the message go; a waiting quit goes on.
    pub fn confirm_discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        log::info!("mail: a message was discarded");
        window.remove_window();
        if self.quitting {
            cx.defer(ely_ui::commands::quit);
        }
    }

    /// Back to writing; a waiting quit stops.
    pub fn keep_writing(&mut self, cx: &mut Context<Self>) {
        self.discarding = false;
        self.quitting = false;
        cx.notify();
    }

    /// Asked on the way out; true lets the app go.
    pub fn may_leave(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let leaves = self.may_close(window, cx);
        self.quitting = self.discarding;
        if !leaves {
            window.activate_window();
        }
        leaves
    }

    fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.may_close(window, cx) {
            log::info!("mail: a blank message closed");
            window.remove_window();
        }
    }

    fn set_markdown(&mut self, markdown: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.markdown = markdown;
        let mode = match markdown {
            true => MarkdownMode::Visual,
            false => MarkdownMode::Source,
        };
        self.document
            .update(cx, |document, cx| document.set_mode(mode, window, cx));
        cx.notify();
    }

    fn pick_files(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let paths = match picked.await {
                Ok(Ok(Some(paths))) => paths,
                Ok(Ok(None)) => return log::info!("mail: the attach panel was cancelled"),
                Ok(Err(error)) => return log::error!("mail: the attach panel failed: {error}"),
                Err(error) => return log::error!("mail: the attach panel went away: {error}"),
            };
            this.update(cx, |compose, cx| compose.attach(paths, cx))
                .ok();
        })
        .detach();
    }

    /// Reads the files off the UI thread, then lists them.
    pub fn attach(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let count = paths.len();
        self.reading += count;
        cx.notify();
        let read = cx.background_spawn(async move {
            paths
                .into_iter()
                .map(|path| {
                    let name = path
                        .file_name()
                        .ok_or_else(|| "a path names no file".to_owned())?
                        .to_string_lossy()
                        .into_owned();
                    let bytes = std::fs::read(&path).map_err(|err| err.to_string())?;
                    Ok(compose::attachment(&name, bytes))
                })
                .collect::<Result<Vec<_>, String>>()
        });
        cx.spawn(async move |this, cx| {
            let read = read.await;
            this.update(cx, |compose, cx| {
                compose.reading -= count;
                match read {
                    Ok(parts) => {
                        log::info!("mail: {} files attached", parts.len());
                        parts.into_iter().for_each(|part| compose.add_file(part));
                    }
                    Err(error) => {
                        log::warn!("mail: a file was not attached");
                        let words = text(cx, "mail.write.attach_failed", &[("error", &error)]);
                        compose.problem = Some((Severity::Danger, words));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn outgoing(&self, draft: &Draft, cx: &App) -> Option<Outgoing> {
        let sender = self.sender()?;
        Some(Outgoing {
            from: Mailbox {
                name: sender.identity.name.clone(),
                email: sender.address.clone(),
            },
            to: addresses(&draft.to),
            cc: addresses(&draft.cc),
            bcc: addresses(&draft.bcc),
            subject: draft.subject.to_string(),
            words: self.document.read(cx).text(cx),
            markdown: self.markdown,
            signature: sender.identity.signature.clone(),
            before: self.before.clone(),
            attachments: self.files.iter().map(|file| file.part.clone()).collect(),
            in_reply_to: self.in_reply_to.clone(),
            references: self.references.clone(),
            date: jiff::Timestamp::now().as_second(),
        })
    }

    pub fn send(&mut self, draft: &Draft, window: &mut Window, cx: &mut Context<Self>) {
        if self.sending {
            return;
        }
        if self.reading > 0 {
            return log::info!("mail: send waits for {} files", self.reading);
        }
        let Some(out) = self.outgoing(draft, cx) else {
            return log::error!("mail: the sender {} is not listed", self.from.0);
        };
        log::info!(
            "mail: sending to {} addresses",
            draft.to.len() + draft.cc.len()
        );
        self.begin_sending(cx);
        let (from, mail) = (self.from, self.mail.clone());
        let built = cx.background_spawn(async move { prepare(&out) });
        cx.spawn_in(window, async move |this, cx| {
            let outcome = async {
                let (envelope, bytes) = built.await.map_err(Sent::from)?;
                let task = mail
                    .update(cx, |mail, _| mail.send_out(from, envelope, bytes))
                    .map_err(|_| Sent::Failed("the mail window is closed".into()))?
                    .map_err(Sent::Failed)?;
                match task.await {
                    Ok(done) => done.map_err(Sent::from),
                    Err(error) => Err(Sent::Failed(error.to_string())),
                }
            }
            .await;
            this.update_in(cx, |compose, window, cx| compose.sent(outcome, window, cx))
                .ok();
        })
        .detach();
    }

    pub fn begin_sending(&mut self, cx: &mut Context<Self>) {
        self.sending = true;
        self.in_flight = Some(self.carried(cx));
        self.problem = None;
        cx.notify();
    }

    pub fn sent(&mut self, outcome: Result<(), Sent>, window: &mut Window, cx: &mut Context<Self>) {
        self.sending = false;
        let carried = self.in_flight.take();
        let (severity, key, error) = match outcome {
            Ok(()) => {
                log::info!("mail: the server accepted a message");
                let words = text(cx, "mail.write.sent", &[]);
                self.mail
                    .update(cx, |mail, cx| mail.toast(words, Severity::Success, cx))
                    .ok();
                return self.after_sending(carried, window, cx);
            }
            Err(Sent::Unsure(error)) => (Severity::Warning, "mail.write.unsure", error),
            Err(Sent::Failed(error)) => (Severity::Danger, "mail.write.failed", error),
        };
        log::warn!("mail: a message was not confirmed sent");
        self.problem = Some((severity, text(cx, key, &[("error", &error)])));
        cx.notify();
    }
}

impl Compose {
    /// Closes, unless edits came after what was sent.
    fn after_sending(
        &mut self,
        carried: Option<Carried>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(carried) = carried else {
            log::error!("mail: a send ended that was never started");
            return;
        };
        if self.reading == 0 && carried == self.carried(cx) {
            return window.remove_window();
        }
        log::info!("mail: edits made while sending stay, unsent");
        self.base = carried;
        self.problem = Some((Severity::Info, text(cx, "mail.write.sent_later", &[])));
        cx.notify();
    }
}

/// What one send carried.
#[derive(PartialEq)]
struct Carried {
    draft: Draft,
    words: String,
    files: Vec<u64>,
    from: AccountId,
    markdown: bool,
}

fn blank() -> Draft {
    Draft {
        to: Vec::new(),
        cc: Vec::new(),
        bcc: Vec::new(),
        subject: SharedString::default(),
    }
}

/// Why the server did not surely accept it.
pub enum Sent {
    /// It went out; whether it was taken is unknown.
    Unsure(String),
    Failed(String),
}

impl From<Error> for Sent {
    fn from(error: Error) -> Self {
        match error {
            Error::Unsure(_) => Sent::Unsure(error.to_string()),
            other => Sent::Failed(other.to_string()),
        }
    }
}

/// The line or lines that introduce the earlier message.
fn lead(how: How, mail: &ely_mail_engine::Mail, words: String, cx: &App) -> Before {
    let sender = people(&mail.from);
    let date = mail.date.map(local_time);
    if how != How::Forward {
        let lead = match &date {
            Some(date) => text(
                cx,
                "mail.write.wrote",
                &[("sender", &sender), ("date", date)],
            ),
            None => text(cx, "mail.write.wrote_undated", &[("sender", &sender)]),
        };
        return Before::Quoted {
            lead: lead.to_string(),
            text: words,
        };
    }
    let line = |key: &str, value: &str| text(cx, key, &[("value", value)]).to_string();
    let mut lead = vec![
        text(cx, "mail.write.forwarded", &[]).to_string(),
        line("mail.write.lead.from", &sender),
    ];
    lead.extend(date.map(|date| line("mail.write.lead.date", &date)));
    lead.extend(
        mail.subject
            .iter()
            .map(|it| line("mail.write.lead.subject", it)),
    );
    lead.push(line("mail.write.lead.to", &people(&mail.to)));
    if !mail.cc.is_empty() {
        lead.push(line("mail.write.lead.cc", &people(&mail.cc)));
    }
    Before::Forwarded { lead, text: words }
}
