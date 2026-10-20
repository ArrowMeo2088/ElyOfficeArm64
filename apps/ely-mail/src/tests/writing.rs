//! Writing: replies, forwards, sending, the sender's identity (C12.12).

use std::path::Path;

use ely_gpui_component::{mail::Draft, primitives::Severity};
use ely_mail_engine::{
    Fetched, Identity, Key, Mailbox, MessageId, Role, Store,
    compose::{Earlier, attachment},
    read,
};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};

use super::{Keys, account, settle, window};
use crate::{
    boxes::Open,
    compose::Compose,
    writing::{How, Outgoing, Purpose, prepare},
};

pub(super) const NOTE: &str = "From: Ann <ann@example.com>\r\nTo: bo@example.com, Cy <cy@example.com>\r\n\
                    Cc: dee@example.com\r\nSubject: plan\r\nDate: 1 Jan 2024 09:00:00 +0000\r\n\
                    Message-ID: <a@x>\r\n";

/// Bo's inbox with one message; returns its id.
pub(super) fn seeded(dir: &Path, bytes: &[u8], missing: &[String]) -> MessageId {
    let mut store = Store::open(dir).unwrap();
    let bo = store.add_account(&account("bo@example.com")).unwrap();
    let [inbox] = store
        .set_folders(bo, &[("INBOX", Some(Role::Inbox))])
        .unwrap()
        .try_into()
        .unwrap();
    let key = Key::Imap {
        uidvalidity: 1,
        uid: 1,
    };
    let fetched = match missing.is_empty() {
        true => Fetched::Whole(bytes),
        false => Fetched::Text { bytes, missing },
    };
    store.put(inbox, &key, &[], fetched).unwrap()
}

pub(super) fn composing(cx: &mut VisualTestContext) -> Vec<WindowHandle<Compose>> {
    cx.update(|_, app| {
        app.windows()
            .into_iter()
            .filter_map(|each| each.downcast::<Compose>())
            .collect()
    })
}

/// Waits for the one compose window.
pub(super) fn opened(cx: &mut VisualTestContext) -> WindowHandle<Compose> {
    settle(cx, |app| {
        app.windows()
            .iter()
            .any(|each| each.downcast::<Compose>().is_some())
    });
    let mut all = composing(cx);
    assert_eq!(all.len(), 1);
    all.remove(0)
}

pub(super) fn draft(to: &[&str], subject: &str) -> Draft {
    Draft {
        to: to.iter().map(|to| to.to_string().into()).collect(),
        cc: Vec::new(),
        bcc: Vec::new(),
        subject: subject.to_owned().into(),
    }
}

#[test]
fn reply_all_starts_addressed_and_quoting_the_message() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = format!("{NOTE}Content-Type: text/html\r\n\r\n<p>Lunch <b>Friday</b>?</p>\r\n");
    let id = seeded(dir.path(), bytes.as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    cx.update(|_, app| mail.update(app, |mail, cx| mail.answer(id, How::ReplyAll, cx)));
    let compose = opened(cx);
    cx.update(|_, app| {
        let compose = compose.read(app).unwrap();
        let shown =
            |list: &[gpui::SharedString]| list.iter().map(|it| it.to_string()).collect::<Vec<_>>();
        assert_eq!(shown(&compose.to), ["ann@example.com", "cy@example.com"]);
        assert_eq!(shown(&compose.cc), ["dee@example.com"]);
        assert_eq!(compose.subject.as_ref(), "Re: plan");
        let Some(Earlier::Quoted { lead, text }) = &compose.before else {
            panic!("the message is quoted");
        };
        assert!(lead.starts_with("On 2024-01-0"), "{lead}");
        assert!(lead.ends_with("Ann <ann@example.com> wrote:"), "{lead}");
        assert_eq!(text, "Lunch Friday?");
    });
    // The quoted reply threads under the original.
    let bytes = cx.update(|_, app| {
        let compose = compose.read(app).unwrap();
        let out = compose
            .outgoing(&draft(&["ann@example.com"], "Re: plan"), app)
            .unwrap();
        prepare(&out, Purpose::Sending).unwrap().1
    });
    let sent = read(&bytes).unwrap();
    assert_eq!(sent.in_reply_to, ["a@x"]);
    assert_eq!(sent.references, ["a@x"]);
    assert!(sent.text.unwrap().contains("> Lunch Friday?"));
}

#[test]
fn a_forward_carries_the_files_that_are_here() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = format!(
        "{NOTE}Content-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nSee both\r\n\
         --b\r\nContent-Type: application/pdf\r\nContent-Disposition: attachment; filename=a.pdf\r\n\r\n%PDF\r\n\
         --b\r\nContent-Type: application/pdf\r\nContent-Disposition: attachment; filename=b.pdf\r\n\r\n\r\n--b--\r\n"
    );
    let id = seeded(dir.path(), bytes.as_bytes(), &["3".to_owned()]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    cx.update(|_, app| mail.update(app, |mail, cx| mail.answer(id, How::Forward, cx)));
    let compose = opened(cx);
    cx.update(|_, app| {
        let compose = compose.read(app).unwrap();
        assert!(compose.to.is_empty());
        assert_eq!(compose.subject.as_ref(), "Fwd: plan");
        let names: Vec<_> = compose
            .files
            .iter()
            .map(|file| file.part.name.clone())
            .collect();
        assert_eq!(names, [Some("a.pdf".to_owned())]);
        assert_eq!(compose.missing, 1, "b.pdf is not downloaded");
        let Some(Earlier::Forwarded { lead, text }) = &compose.before else {
            panic!("the message is forwarded");
        };
        assert!(
            lead.contains(&"From: Ann <ann@example.com>".to_owned()),
            "{lead:?}"
        );
        assert!(lead.contains(&"Cc: dee@example.com".to_owned()), "{lead:?}");
        assert_eq!(text, "See both");
    });
    let mut there = VisualTestContext::from_window(compose.into(), cx);
    settle(&mut there, |_| true);
    assert!(there.debug_bounds("write-missing").is_some());
}

#[test]
fn a_message_is_built_with_its_hidden_copies_off_the_page() {
    let mailbox = |email: &str| Mailbox {
        name: None,
        email: email.into(),
    };
    let out = Outgoing {
        from: Mailbox {
            name: Some("Bo Lee".into()),
            email: "bo@example.com".into(),
        },
        to: vec![mailbox("ann@example.com")],
        cc: vec![mailbox("ANN@example.com"), mailbox("cy@example.com")],
        bcc: vec![mailbox("hidden@example.com")],
        subject: "Plans".into(),
        words: "**Friday** then".into(),
        markdown: true,
        signature: "Bo".into(),
        before: None,
        attachments: vec![attachment("plan.pdf", b"%PDF".to_vec())],
        in_reply_to: None,
        references: Vec::new(),
        date: 1_704_099_600,
    };
    let (envelope, bytes) = prepare(&out, Purpose::Sending).unwrap();
    assert_eq!(
        envelope,
        ["ann@example.com", "cy@example.com", "hidden@example.com"]
    );
    let raw = String::from_utf8_lossy(&bytes);
    assert!(!raw.to_ascii_lowercase().contains("bcc"), "{raw}");
    let sent = read(&bytes).unwrap();
    assert_eq!(sent.from[0].name.as_deref(), Some("Bo Lee"));
    let text = sent.text.unwrap().replace("\r\n", "\n");
    assert_eq!(text.trim_end(), "**Friday** then\n\n-- \nBo");
    assert!(sent.html.unwrap().contains("<strong>Friday</strong>"));
    assert_eq!(sent.attachments[0].name.as_deref(), Some("plan.pdf"));
    let plain = Outgoing {
        markdown: false,
        ..out
    };
    assert_eq!(
        read(&prepare(&plain, Purpose::Sending).unwrap().1)
            .unwrap()
            .html,
        None
    );
    let (_, copy) = prepare(&plain, Purpose::Draft).unwrap();
    let copy = String::from_utf8_lossy(&copy);
    let head = &copy[..copy.find("\r\n\r\n").unwrap()];
    assert!(
        head.contains("hidden@example.com"),
        "a draft keeps Bcc: {head}"
    );
    assert!(
        head.lines()
            .any(|line| line.to_ascii_lowercase().starts_with("bcc:")),
        "{head}"
    );
}

#[test]
fn words_written_are_kept_on_closing() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    cx.update(|_, app| mail.update(app, |mail, cx| mail.write_new(cx)));
    let compose = opened(cx);
    let closes = |cx: &mut VisualTestContext| {
        cx.update(|_, app| {
            compose
                .update(app, |compose, window, cx| compose.may_close(window, cx))
                .unwrap()
        })
    };
    assert!(closes(cx), "a blank message closes");
    cx.update(|_, app| {
        compose
            .update(app, |compose, _, cx| {
                compose
                    .document
                    .update(cx, |document, cx| document.set_text("Hello", cx))
            })
            .unwrap()
    });
    assert!(!closes(cx), "words are kept first");
    settle(cx, |app| compose.read(app).is_err());
}

#[test]
fn a_send_that_cannot_reach_the_server_says_so_and_stays() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let keys = Keys::default();
    keys.kept
        .lock()
        .unwrap()
        .insert("account:bo@example.com".into(), "pw".into());
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), keys);
    cx.update(|_, app| mail.update(app, |mail, cx| mail.write_new(cx)));
    let compose = opened(cx);
    cx.update(|_, app| {
        compose
            .update(app, |compose, window, cx| {
                compose.send(&draft(&["ann@example.com"], "Hi"), window, cx);
                assert!(compose.sending);
            })
            .unwrap()
    });
    settle(cx, |app| {
        compose.read(app).is_ok_and(|compose| !compose.sending)
    });
    cx.update(|_, app| {
        let compose = compose.read(app).unwrap();
        let (severity, words) = compose.problem.clone().unwrap();
        assert_eq!(severity, Severity::Danger);
        assert!(words.starts_with("Not sent:"), "{words}");
    });
    assert_eq!(composing(cx).len(), 1, "the window stays for another try");
}

#[test]
fn a_saved_name_and_signature_sign_the_next_message() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    let bo = cx.update(|_, app| mail.read(app).accounts()[0].0);
    cx.update(|_, app| mail.update(app, |mail, cx| mail.open_box(Open::Account(bo), cx)));
    settle(cx, |app| {
        mail.read(app)
            .identity
            .as_ref()
            .is_some_and(|editing| editing.kept.is_some())
    });
    assert!(cx.debug_bounds("mail-identity").is_some());
    let signed = Identity {
        name: Some("Bo Lee".into()),
        signature: "Bo\nAtrium".into(),
    };
    cx.update(|_, app| mail.update(app, |mail, cx| mail.save_identity(bo, signed.clone(), cx)));
    settle(cx, |_| {
        Store::open(dir.path()).unwrap().identity(bo).unwrap() == signed
    });
    cx.update(|_, app| mail.update(app, |mail, cx| mail.write_new(cx)));
    let compose = opened(cx);
    cx.update(|_, app| {
        let compose = compose.read(app).unwrap();
        assert_eq!(compose.from, bo);
        assert_eq!(compose.senders[0].identity, signed);
        let out = compose
            .outgoing(&draft(&["ann@example.com"], "Hi"), app)
            .unwrap();
        let sent = read(&prepare(&out, Purpose::Sending).unwrap().1).unwrap();
        assert_eq!(sent.from[0].name.as_deref(), Some("Bo Lee"));
        let text = sent.text.unwrap().replace("\r\n", "\n");
        assert!(text.ends_with("-- \nBo\nAtrium"), "{text:?}");
    });
}

#[test]
fn a_message_kept_as_headers_is_answered_without_quoting() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let bo = store.add_account(&account("bo@example.com")).unwrap();
    let [inbox] = store
        .set_folders(bo, &[("INBOX", Some(Role::Inbox))])
        .unwrap()
        .try_into()
        .unwrap();
    let key = Key::Imap {
        uidvalidity: 1,
        uid: 1,
    };
    let head = format!("{NOTE}\r\n");
    let id = store
        .put(inbox, &key, &[], Fetched::Headers(head.as_bytes()))
        .unwrap();
    drop(store);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    cx.update(|_, app| mail.update(app, |mail, cx| mail.answer(id, How::Reply, cx)));
    let compose = opened(cx);
    cx.update(|_, app| {
        let compose = compose.read(app).unwrap();
        assert_eq!(compose.subject.as_ref(), "Re: plan");
        assert!(compose.headers_only);
        assert!(compose.before.is_none(), "no words to quote");
    });
}
