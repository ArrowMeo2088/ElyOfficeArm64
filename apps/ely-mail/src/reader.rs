//! The reading pane (C12.10).

use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    documents::{MarkdownBlock, MarkdownRenderer},
    feedback::{Alert, EmptyState, ErrorView},
    layout::ScrollArea,
    mail::{Contact, MailReader, Message},
    primitives::{IconName, Severity},
};
use ely_mail_engine::Mailbox;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, Styled as _, div,
};

use crate::{
    letter::Body,
    mail::{Mail, text},
    reading::{Open, Shown},
};

fn contact(mailbox: &Mailbox) -> Contact {
    let name = match &mailbox.name {
        Some(name) if !name.trim().is_empty() => name.clone(),
        _ => mailbox.email.clone(),
    };
    Contact::new(name, mailbox.email.clone())
}

impl Mail {
    pub fn reader(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(shown) = &self.reading.shown else {
            let none = text(cx, "mail.reader.none", &[]);
            return EmptyState::new("mail-reader-empty", IconName::Mail, none).into_any_element();
        };
        let open = match &shown.letter {
            None => {
                let words = text(cx, "mail.reader.reading", &[]);
                return EmptyState::new("mail-reader-reading", IconName::Mail, words)
                    .into_any_element();
            }
            Some(Err(error)) => {
                let words = text(cx, "mail.reader.failed", &[]);
                return ErrorView::new("mail-reader-error", IconName::TriangleAlert, words)
                    .detail(error.clone())
                    .into_any_element();
            }
            Some(Ok(open)) => open,
        };
        ScrollArea::new("mail-reader-scroll")
            .vertical()
            .size_full()
            .child(div().p_6().child(self.letter(shown, open, cx)))
            .into_any_element()
    }

    fn letter(&self, shown: &Shown, open: &Open, cx: &mut Context<Self>) -> AnyElement {
        let letter = &open.letter;
        let from = match &letter.from {
            Some(from) => contact(from),
            None => Contact::new(text(cx, "mail.no_sender", &[]), ""),
        };
        let key = shown.id.0.to_string();
        let message = match letter.date.map(jiff::Timestamp::from_second) {
            Some(Ok(at)) => Message::new(key, from, at),
            Some(Err(error)) => {
                log::warn!("mail: message {} date unshown: {error}", shown.id.0);
                Message::undated(key, from)
            }
            None => Message::undated(key, from),
        };
        let mut message = message
            .to(letter.to.iter().map(contact))
            .cc(letter.cc.iter().map(contact));
        for (name, size) in letter.body.iter().flat_map(|body| &body.files) {
            message = match size {
                Some(size) => message.file(name.clone(), *size),
                None => message.file_waiting(name.clone()),
            };
        }
        let subject = match &letter.subject {
            Some(subject) => subject.clone().into(),
            None => text(cx, "mail.no_subject", &[]),
        };
        let reader = MailReader::new("mail-reader", subject, message);
        let Some(body) = &letter.body else {
            let only = text(cx, "mail.reader.headers_only", &[]);
            return reader
                .child(Alert::new("mail-headers-only", Severity::Info, only))
                .child(self.headers(shown, &letter.headers, cx))
                .into_any_element();
        };
        let drawn = MarkdownRenderer::blocks("mail-body", open.blocks.clone())
            .pictures(open.pictures.clone())
            .on_link(|url, _, cx| {
                // The engine kept only web and mail links.
                log::info!("mail: a link opens");
                cx.open_url(url);
            });
        reader
            .children(self.pictures_notice(open, body, cx))
            .children(body.short.then(|| {
                let cut = text(cx, "mail.reader.short", &[]);
                Alert::new("mail-short", Severity::Warning, cut)
            }))
            .child(drawn)
            .child(self.headers(shown, &letter.headers, cx))
            .into_any_element()
    }

    /// What web pictures wait, and how to let them in.
    fn pictures_notice(&self, open: &Open, body: &Body, cx: &mut Context<Self>) -> Option<Alert> {
        let sender = open.sender().unwrap_or_default().to_owned();
        let trackers = body.trackers.to_string();
        let kept =
            (body.trackers > 0).then(|| text(cx, "mail.reader.trackers", &[("n", &trackers)]));
        if open.trusted {
            let title = text(cx, "mail.reader.trusted", &[("sender", &sender)]);
            let this = cx.weak_entity();
            let stop = Button::new("mail-distrust", text(cx, "mail.reader.distrust", &[]))
                .variant(ButtonVariant::Ghost)
                .on_click(move |_, _, cx| {
                    this.update(cx, |mail, cx| mail.trust_sender(false, cx))
                        .ok();
                });
            let alert = Alert::new("mail-trusted", Severity::Info, title).action(stop);
            return Some(kept.into_iter().fold(alert, Alert::body));
        }
        if body.remote.is_empty() || open.fetching {
            return kept.map(|kept| Alert::new("mail-trackers", Severity::Info, kept));
        }
        let count = body.remote.len().to_string();
        let title = text(cx, "mail.reader.blocked", &[("n", &count)]);
        let this = cx.weak_entity();
        let load = Button::new("mail-load-pictures", text(cx, "mail.reader.load", &[]))
            .variant(ButtonVariant::Primary)
            .on_click(move |_, _, cx| {
                this.update(cx, |mail, cx| mail.load_pictures(cx)).ok();
            });
        let mut alert = Alert::new("mail-blocked", Severity::Info, title).action(load);
        if !sender.is_empty() {
            let this = cx.weak_entity();
            let words = text(cx, "mail.reader.trust", &[("sender", &sender)]);
            alert = alert.action(Button::new("mail-trust", words).on_click(move |_, _, cx| {
                this.update(cx, |mail, cx| mail.trust_sender(true, cx)).ok();
            }));
        }
        Some(kept.into_iter().fold(alert, Alert::body))
    }

    /// The header block as it came, on request.
    fn headers(&self, shown: &Shown, headers: &str, cx: &mut Context<Self>) -> AnyElement {
        let key = if shown.headers {
            "mail.reader.headers.hide"
        } else {
            "mail.reader.headers.show"
        };
        let this = cx.weak_entity();
        let toggle = Button::new("mail-headers", text(cx, key, &[]))
            .variant(ButtonVariant::Ghost)
            .on_click(move |_, _, cx| {
                this.update(cx, |mail, cx| mail.toggle_headers(cx)).ok();
            });
        let toggle = div().debug_selector(|| "mail-headers".into()).child(toggle);
        let mut part = div().flex().flex_col().gap_2().child(toggle);
        if shown.headers {
            let code: std::rc::Rc<[MarkdownBlock]> = vec![MarkdownBlock::Code {
                language: None,
                text: headers.trim_end().to_owned(),
                from: 0,
            }]
            .into();
            part = part.child(MarkdownRenderer::blocks("mail-headers-text", code));
        }
        part.into_any_element()
    }
}
