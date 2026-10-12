use std::sync::{Arc, Mutex};

use gpui::{AppContext as _, Entity, TestAppContext, VisualTestContext};

use super::{MarkView, Mode, Reported};
use crate::engine::{Text, mark};
use crate::text::Source;

fn open<'a>(
    text: &str,
    cx: &'a mut TestAppContext,
) -> (Entity<MarkView>, &'a mut VisualTestContext) {
    cx.update(ely_gpui_component::init_for_tests);
    cx.update(|cx| {
        ely_ui::settings::install_defaults(cx);
        ely_ui::i18n::install(cx);
    });
    let source = Source::decode(text.as_bytes().to_vec()).expect("reads");
    let saved = mark(&source.text);
    let doc = Arc::new(Mutex::new(Text::new(source)));
    let folder = std::env::temp_dir();
    let (view, cx) =
        cx.add_window_view(move |window, cx| MarkView::new(doc, saved, folder, window, cx));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    (view, cx)
}

#[gpui::test]
fn typing_reaches_the_text_the_shell_saves(cx: &mut TestAppContext) {
    let (view, cx) = open("# A\n\nB\n", cx);
    let outline: Vec<(usize, String, usize)> = view.read_with(cx, |view, cx| {
        view.outline(cx)
            .into_iter()
            .map(|heading| (heading.level, heading.title, heading.line))
            .collect()
    });
    assert_eq!(outline, [(1, "A".to_string(), 0)]);
    view.update_in(cx, |view, window, cx| view.switch(Mode::Source, window, cx));
    let focus = view.read_with(cx, |view, _| view.focus.clone());
    cx.update(|window, cx| window.focus(&focus, cx));
    cx.run_until_parked();
    cx.simulate_input("x");
    let (bytes, modified, changed) = view.read_with(cx, |view, _| {
        (view.bytes(), view.modified(), view.changed())
    });
    assert_eq!(bytes, "x# A\n\nB\n".len());
    assert!(modified && changed > 0);
}

fn current(view: &Entity<MarkView>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |view, _| {
        view.doc.lock().expect("the text lock").current.clone()
    })
}

#[gpui::test]
fn a_refresh_is_saved_text_and_keeps_no_undo(cx: &mut TestAppContext) {
    let (view, cx) = open("one\n", cx);
    view.update(cx, |view, cx| view.adopt("edited\n".into(), cx));
    cx.run_until_parked();
    assert!(view.read_with(cx, |view, _| view.modified()));
    let disk = Source::decode(b"from disk\n".to_vec()).expect("reads");
    view.update(cx, |view, cx| view.refresh(disk, cx));
    cx.run_until_parked();
    assert_eq!(current(&view, cx), "from disk\n");
    assert!(!view.read_with(cx, |view, _| view.modified()));
    view.update_in(cx, |view, window, cx| view.switch(Mode::Source, window, cx));
    let focus = view.read_with(cx, |view, _| view.focus.clone());
    cx.update(|window, cx| window.focus(&focus, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("secondary-z");
    cx.run_until_parked();
    assert_eq!(current(&view, cx), "from disk\n", "undo stops at the disk");
}

#[gpui::test]
fn the_shell_hears_a_load_it_subscribed_to(cx: &mut TestAppContext) {
    cx.update(ely_gpui_component::init_for_tests);
    cx.update(|cx| {
        ely_ui::settings::install_defaults(cx);
        ely_ui::i18n::install(cx);
    });
    let heard = std::rc::Rc::new(std::cell::Cell::new(0));
    let (count, held) = (
        heard.clone(),
        std::rc::Rc::new(std::cell::RefCell::new(None)),
    );
    let holder = held.clone();
    let (_root, cx) = cx.add_window_view(move |window, cx| {
        let source = Source::decode(b"# Opened\n".to_vec()).expect("reads");
        let saved = mark(&source.text);
        let doc = Arc::new(Mutex::new(Text::new(source)));
        let folder = std::env::temp_dir();
        let view = cx.new(|cx| MarkView::new(doc, saved, folder, window, cx));
        // As the shell: subscribed after the view is made.
        let count = count.clone();
        cx.subscribe(&view, move |_, _, _: &Reported, _| {
            count.set(count.get() + 1)
        })
        .detach();
        holder.replace(Some(view));
        gpui::Empty
    });
    cx.run_until_parked();
    assert!(heard.get() > 0);
}

#[gpui::test]
fn a_text_grown_past_the_limit_goes_to_source(cx: &mut TestAppContext) {
    let (view, cx) = open("# Old\n", cx);
    let long = "x".repeat(ely_gpui_component::documents::LARGEST_READ + 1);
    view.update(cx, |view, cx| view.adopt(long, cx));
    cx.run_until_parked();
    assert!(view.read_with(cx, |view, _| view.large()));
    assert_eq!(view.read_with(cx, |view, _| view.mode), Mode::Source);
    view.update_in(cx, |view, window, cx| {
        view.switch(Mode::Preview, window, cx)
    });
    assert_eq!(view.read_with(cx, |view, _| view.mode), Mode::Source);
    let outline = view.read_with(cx, |view, cx| view.outline(cx));
    assert!(outline.is_empty(), "no stale heading: {outline:?}");
}

#[gpui::test]
fn a_large_file_stays_in_source(cx: &mut TestAppContext) {
    let large = ely_gpui_component::documents::LARGEST_READ / 2 + 1;
    let (view, cx) = open(&"a\n".repeat(large), cx);
    assert_eq!(view.read_with(cx, |view, _| view.mode), Mode::Source);
    view.update_in(cx, |view, window, cx| view.switch(Mode::Visual, window, cx));
    assert_eq!(view.read_with(cx, |view, _| view.mode), Mode::Source);
}

#[gpui::test]
fn an_edit_keeps_the_rest_of_the_file_byte_for_byte(cx: &mut TestAppContext) {
    let bytes = b"\xEF\xBB\xBF# A\r\n\r\nB\r\n".to_vec();
    let (view, cx) = open(std::str::from_utf8(&bytes).expect("UTF-8"), cx);
    view.update_in(cx, |view, window, cx| view.switch(Mode::Source, window, cx));
    let focus = view.read_with(cx, |view, _| view.focus.clone());
    cx.update(|window, cx| window.focus(&focus, cx));
    cx.run_until_parked();
    cx.simulate_input("x");
    let written = view.read_with(cx, |view, _| {
        let doc = view.doc.lock().expect("the text lock");
        doc.source.encode(&doc.current)
    });
    assert_eq!(written, b"\xEF\xBB\xBFx# A\r\n\r\nB\r\n");
}
