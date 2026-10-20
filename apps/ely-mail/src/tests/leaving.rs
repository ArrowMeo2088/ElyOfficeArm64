//! Leaving with messages open; files being read; edits (C12.12).

use ely_gpui_component::{mail::Draft, primitives::Severity};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};

use super::{
    Keys, settle, window,
    writing::{NOTE, composing, draft, opened, seeded},
};
use crate::{
    compose::{Compose, file_name},
    writing::{How, Sender, guard_leaving, prepare},
};
use ely_mail_engine::AccountId;

/// A message with a PDF here and one not downloaded.
fn with_files() -> String {
    format!(
        "{NOTE}Content-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nSee\r\n\
         --b\r\nContent-Type: application/pdf\r\nContent-Disposition: attachment; filename=a.pdf\r\n\r\n%PDF\r\n--b--\r\n"
    )
}

fn closes(compose: WindowHandle<Compose>, cx: &mut VisualTestContext) -> bool {
    cx.update(|_, app| {
        compose
            .update(app, |compose, window, cx| {
                let closes = compose.may_close(window, cx);
                compose.discarding = false;
                closes
            })
            .unwrap()
    })
}

fn with<R>(
    compose: WindowHandle<Compose>,
    cx: &mut VisualTestContext,
    act: impl FnOnce(&mut Compose, &mut gpui::Window, &mut gpui::Context<Compose>) -> R,
) -> R {
    cx.update(|_, app| compose.update(app, act).unwrap())
}

fn write(compose: WindowHandle<Compose>, words: &str, cx: &mut VisualTestContext) {
    with(compose, cx, |compose, _, cx| {
        compose
            .document
            .update(cx, |document, cx| document.set_text(words, cx))
    });
}

#[test]
fn quit_asks_each_message_with_words_and_goes_on_after_a_discard() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    cx.update(|window, app| {
        ely_ui::commands::bind_quit(app);
        guard_leaving(window, app);
    });
    for count in 1..=2 {
        cx.update(|_, app| mail.update(app, |mail, cx| mail.write_new(cx)));
        settle(cx, |app| {
            let open = app.windows().into_iter();
            open.filter(|each| each.downcast::<Compose>().is_some())
                .count()
                == count
        });
    }
    let [first, second] = composing(cx).try_into().unwrap();
    write(first, "one", cx);
    write(second, "two", cx);
    cx.update(|window, app| window.dispatch_action(Box::new(ely_ui::commands::Quit), app));
    settle(cx, |_| true);
    let asked = |compose: WindowHandle<Compose>, cx: &mut VisualTestContext| {
        cx.update(|_, app| compose.read(app).unwrap().discarding)
    };
    assert!(asked(first, cx), "the first message asks");
    assert!(!asked(second, cx), "one question at a time");
    assert_eq!(composing(cx).len(), 2, "nothing closed yet");
    with(first, cx, |compose, window, cx| {
        compose.confirm_discard(window, cx)
    });
    settle(cx, |app| {
        let open = app.windows().into_iter();
        open.filter(|each| each.downcast::<Compose>().is_some())
            .count()
            == 1
    });
    assert!(asked(second, cx), "the quit went on to the next");
    with(second, cx, |compose, _, cx| compose.keep_writing(cx));
    assert!(!asked(second, cx));
}

#[test]
fn the_main_window_stays_while_a_message_is_sending() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    cx.update(guard_leaving);
    cx.update(|_, app| mail.update(app, |mail, cx| mail.write_new(cx)));
    let compose = opened(cx);
    with(compose, cx, |compose, _, _| compose.sending = true);
    assert!(!cx.simulate_close(), "the main window stays");
    let problem = cx.update(|_, app| compose.read(app).unwrap().problem.clone());
    assert_eq!(
        problem.map(|(severity, _)| severity),
        Some(Severity::Warning)
    );
}

#[test]
fn send_waits_for_files_still_being_read() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let file = dir.path().join("note.txt");
    std::fs::write(&file, "hello").unwrap();
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    cx.update(|_, app| mail.update(app, |mail, cx| mail.write_new(cx)));
    let compose = opened(cx);
    with(compose, cx, |compose, window, cx| {
        compose.attach(vec![file], cx);
        compose.send(&draft(&["ann@example.com"], "Hi"), window, cx);
        assert!(!compose.sending, "the file is not read yet");
        assert_eq!(compose.reading, 1);
    });
    assert!(!closes(compose, cx), "a file on its way asks first");
    settle(cx, |app| compose.read(app).unwrap().reading == 0);
    cx.update(|_, app| assert_eq!(compose.read(app).unwrap().files.len(), 1));
}

#[test]
fn a_forward_asks_once_its_addresses_subject_or_files_change() {
    let dir = tempfile::tempdir().unwrap();
    let id = seeded(dir.path(), with_files().as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    let forward = |cx: &mut VisualTestContext| {
        cx.update(|_, app| mail.update(app, |mail, cx| mail.answer(id, How::Forward, cx)));
        let compose = opened(cx);
        settle(cx, |_| true);
        compose
    };
    let compose = forward(cx);
    assert!(closes(compose, cx), "untouched, it closes");
    let addressed = Draft {
        to: vec!["eve@example.com".into()],
        ..draft(&[], "Fwd: plan")
    };
    with(compose, cx, |compose, _, cx| compose.edited(addressed, cx));
    assert!(!closes(compose, cx), "an address makes it ask");
    with(compose, cx, |compose, _, cx| {
        compose.edited(draft(&[], "Fwd: plan!"), cx)
    });
    assert!(!closes(compose, cx), "a subject makes it ask");
    with(compose, cx, |compose, _, cx| {
        compose.edited(draft(&[], "Fwd: plan"), cx)
    });
    assert!(closes(compose, cx), "back as it was");
    with(compose, cx, |compose, _, _| compose.files.clear());
    assert!(!closes(compose, cx), "a forwarded file taken off asks");
}

#[test]
fn a_file_without_a_name_is_forwarded() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = format!(
        "{NOTE}Content-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nSee\r\n\
         --b\r\nContent-Type: application/pdf\r\nContent-Disposition: attachment\r\n\r\n%PDF\r\n--b--\r\n"
    );
    let id = seeded(dir.path(), bytes.as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    cx.update(|_, app| mail.update(app, |mail, cx| mail.answer(id, How::Forward, cx)));
    let compose = opened(cx);
    let out = cx.update(|_, app| {
        let compose = compose.read(app).unwrap();
        assert_eq!(compose.files.len(), 1);
        assert_eq!(
            file_name(&compose.files[0].part, app).as_ref(),
            "Unnamed file"
        );
        compose.outgoing(&draft(&["eve@example.com"], "Fwd: plan"), app)
    });
    let (_, bytes) = prepare(&out.unwrap()).unwrap();
    let sent = ely_mail_engine::read(&bytes).unwrap();
    assert_eq!(sent.attachments.len(), 1);
    assert_eq!(sent.attachments[0].name, None);
}

/// Presses the drawn Close of `on`, Windows and Linux style.
fn press_close(on: gpui::AnyWindowHandle, cx: &mut VisualTestContext) {
    let mut there = VisualTestContext::from_window(on, cx);
    there.run_until_parked();
    let controls = there
        .debug_bounds("titlebar-controls")
        .expect("drawn controls");
    let close = gpui::point(controls.right() - gpui::px(8.0), controls.center().y);
    there.simulate_click(close, gpui::Modifiers::none());
    there.run_until_parked();
}

#[test]
fn the_drawn_close_asks_as_the_system_close_does() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    let main = cx.update(|window, _| window.window_handle());
    cx.update(guard_leaving);
    cx.update(|_, app| mail.update(app, |mail, cx| mail.write_new(cx)));
    let compose = opened(cx);
    write(compose, "words", cx);
    press_close(compose.into(), cx);
    let asked = cx.update(|_, app| compose.read(app).map(|compose| compose.discarding));
    assert_eq!(asked.ok(), Some(true), "the message stays and asks");
    with(compose, cx, |compose, _, cx| compose.keep_writing(cx));
    press_close(main, cx);
    let open = cx.update(|_, app| app.windows().len());
    assert_eq!(open, 2, "the main window waits for the message");
}

#[test]
fn a_success_keeps_the_window_holding_edits_made_while_sending() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    for late in [false, true] {
        cx.update(|_, app| mail.update(app, |mail, cx| mail.write_new(cx)));
        let compose = opened(cx);
        write(compose, "before", cx);
        with(compose, cx, |compose, _, cx| {
            compose.edited(draft(&["ann@example.com"], "Hi"), cx);
            compose.begin_sending(cx);
        });
        if late {
            write(compose, "before and late edit", cx);
        }
        with(compose, cx, |compose, window, cx| {
            compose.sent(Ok(()), window, cx)
        });
        settle(cx, |_| true);
        if !late {
            assert!(composing(cx).is_empty(), "as sent, it closes");
            continue;
        }
        let problem = cx.update(|_, app| compose.read(app).unwrap().problem.clone());
        assert_eq!(problem.map(|(severity, _)| severity), Some(Severity::Info));
        assert!(!closes(compose, cx), "the late edit asks before it goes");
        write(compose, "before", cx);
        assert!(closes(compose, cx), "as sent, nothing is lost");
        with(compose, cx, |_, window, _| window.remove_window());
        settle(cx, |_| true);
    }
}

#[test]
fn a_success_keeps_the_window_when_the_sender_or_format_changed() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path(), format!("{NOTE}\r\nx").as_bytes(), &[]);
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    for sender in [false, true] {
        cx.update(|_, app| mail.update(app, |mail, cx| mail.write_new(cx)));
        let compose = opened(cx);
        write(compose, "words", cx);
        with(compose, cx, |compose, _, cx| {
            let other = Sender {
                id: AccountId(compose.from.0 + 1),
                address: "other@example.com".into(),
                identity: compose.senders[0].identity.clone(),
            };
            compose.senders.push(other);
            compose.edited(draft(&["ann@example.com"], "Hi"), cx);
            compose.begin_sending(cx);
        });
        if sender {
            with(compose, cx, |compose, _, _| compose.from = compose.senders[1].id);
        } else {
            let mut there = VisualTestContext::from_window(compose.into(), cx);
            there.run_until_parked();
            let plain = there.debug_bounds("segment plain").expect("the format switch");
            there.simulate_click(plain.center(), gpui::Modifiers::none());
            there.run_until_parked();
            assert!(!cx.update(|_, app| compose.read(app).unwrap().markdown));
        }
        with(compose, cx, |compose, window, cx| {
            compose.sent(Ok(()), window, cx)
        });
        settle(cx, |_| true);
        let problem = cx.update(|_, app| compose.read(app).map(|it| it.problem.clone()));
        let severity = problem.ok().flatten().map(|(severity, _)| severity);
        assert_eq!(severity, Some(Severity::Info), "sender changed: {sender}");
        assert!(!closes(compose, cx), "the unsent change asks first");
        with(compose, cx, |_, window, _| window.remove_window());
        settle(cx, |_| true);
    }
}
