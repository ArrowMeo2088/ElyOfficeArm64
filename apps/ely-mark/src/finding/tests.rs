use ely_gpui_component::forms::TextInput;
use gpui::{AppContext as _, Entity, TestAppContext, VisualTestContext};

use crate::desk::{
    Desk,
    tests::{open, start},
};

fn opened(
    text: &str,
    cx: &mut TestAppContext,
) -> (
    Entity<Desk>,
    &'static mut VisualTestContext,
    [tempfile::TempDir; 2],
) {
    let (data, files) = (
        tempfile::tempdir().expect("data"),
        tempfile::tempdir().expect("files"),
    );
    let path = files.path().join("a.md");
    std::fs::write(&path, text).expect("a file");
    let (desk, cx) = start(cx, data.path());
    open(&desk, &path, cx);
    (desk, cx, [data, files])
}

fn query(desk: &Entity<Desk>, words: &str, cx: &mut VisualTestContext) {
    desk.update_in(cx, |desk, window, cx| {
        desk.on_find(&ely_office::FindReplace, window, cx)
    });
    cx.simulate_input(words);
    cx.run_until_parked();
}

fn current_text(desk: &Entity<Desk>, cx: &mut VisualTestContext) -> String {
    desk.read_with(cx, |desk, cx| {
        let view = desk.active_view().expect("a tab");
        view.read(cx).document().read(cx).text(cx)
    })
}

#[gpui::test]
fn every_match_is_counted_and_replaced_in_one_step(cx: &mut TestAppContext) {
    let (desk, cx, _files) = opened("a b a\n", cx);
    query(&desk, "a", cx);
    assert_eq!(
        desk.read_with(cx, |desk, _| desk
            .finding
            .as_ref()
            .map(|find| find.hits.len())),
        Some(2)
    );
    desk.update_in(cx, |desk, window, cx| {
        let field = cx.new(|cx| TextInput::new(window, cx));
        field.update(cx, |field, cx| field.set_text("x", cx));
        desk.finding.as_mut().expect("open").replace = Some(field);
        desk.replace(true, window, cx);
    });
    cx.run_until_parked();
    assert_eq!(current_text(&desk, cx), "x b x\n");
    let focus = desk.read_with(cx, |desk, cx| {
        desk.active_view().expect("a tab").read(cx).focus.clone()
    });
    cx.update(|window, cx| window.focus(&focus, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("secondary-z");
    assert_eq!(
        current_text(&desk, cx),
        "a b a\n",
        "one undo takes all back"
    );
}

#[gpui::test]
fn a_bad_pattern_says_why(cx: &mut TestAppContext) {
    let (desk, cx, _files) = opened("a\n", cx);
    desk.update_in(cx, |desk, window, cx| {
        desk.on_find(&ely_office::FindReplace, window, cx)
    });
    desk.update(cx, |desk, cx| {
        desk.finding.as_mut().expect("open").options.regex = true;
        desk.count(cx);
    });
    cx.simulate_input("(");
    cx.run_until_parked();
    let error = desk.read_with(cx, |desk, _| {
        desk.finding.as_ref().and_then(|find| find.error.clone())
    });
    assert!(error.is_some());
}

#[gpui::test]
fn a_step_shows_the_next_match(cx: &mut TestAppContext) {
    let (desk, cx, _files) = opened("one\n\ntwo one\n", cx);
    query(&desk, "one", cx);
    desk.update_in(cx, |desk, window, cx| {
        desk.step(true, window, cx);
        desk.step(true, window, cx);
    });
    let current = desk.read_with(cx, |desk, _| {
        desk.finding.as_ref().and_then(|find| find.current)
    });
    assert_eq!(current, Some(1));
}

#[gpui::test]
fn the_table_menu_inserts_and_edits_a_table(cx: &mut TestAppContext) {
    let (desk, cx, _files) = opened("Intro\n", cx);
    let focus = desk.read_with(cx, |desk, _| desk.core.focus.clone());
    cx.update(|window, cx| window.focus(&focus, cx));
    cx.dispatch_action(crate::tools::InsertTable);
    assert!(current_text(&desk, cx).contains("| --- | --- | --- |"));
    cx.dispatch_action(crate::tools::DeleteColumn);
    assert!(
        current_text(&desk, cx).contains("| --- | --- |\n"),
        "{}",
        current_text(&desk, cx)
    );
}
