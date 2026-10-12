use std::sync::{Arc, Mutex};

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::{MarkView, Mode};
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

#[gpui::test]
fn a_refresh_is_saved_text_and_keeps_no_undo(cx: &mut TestAppContext) {
    let (view, cx) = open("one\n", cx);
    let disk = Source::decode(b"two\n".to_vec()).expect("reads");
    view.update(cx, |view, cx| view.refresh(disk, cx));
    cx.run_until_parked();
    assert!(!view.read_with(cx, |view, _| view.modified()));
    assert_eq!(view.read_with(cx, |view, _| view.bytes()), 4);
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
