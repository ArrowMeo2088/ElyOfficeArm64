//! Password prompts, zoomed pairs, jumps across (C11.1).

use ely_office::{Dialog, Dialogs as _, Tabs as _};
use gpui::{Entity, TestAppContext, VisualTestContext, point, px};

use super::{file, frames, settle, shown, windowed};
use crate::layout::{Mode, Zoom};
use crate::view::PdfView;
use crate::viewer::Viewer;

/// The file the open prompt asks for.
fn asking(app: &gpui::App, viewer: &Entity<Viewer>) -> Option<std::path::PathBuf> {
    match &viewer.read(app).core.dialog {
        Some(Dialog::Password { path, .. }) => Some(path.clone()),
        _ => None,
    }
}

/// Two files asked for at once, as locked loads do.
fn asked<'a>(cx: &'a mut TestAppContext, dir: &std::path::Path) -> (Entity<Viewer>, &'a mut VisualTestContext, [std::path::PathBuf; 2]) {
    let (viewer, cx) = windowed(cx, dir, &[], true);
    let paths = [file(dir, "a.pdf", &[(612.0, 792.0, &["a"])]), file(dir, "b.pdf", &[(612.0, 792.0, &["b"])])];
    cx.update(|window, app| viewer.update(app, |viewer, cx| {
        for path in &paths {
            viewer.ask_password(path.clone(), false, false, window, cx);
        }
    }));
    frames(cx);
    (viewer, cx, paths)
}

#[test]
fn a_typed_password_opens_the_file() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let (viewer, cx, paths) = asked(&mut cx, dir.path());
    // An open file takes any password.
    cx.simulate_input("secret");
    cx.simulate_keystrokes("enter");
    settle(cx, |app| viewer.read(app).core.tabs.len() == 1);
    assert_eq!(cx.update(|_, app| asking(app, &viewer)), Some(paths[1].clone()), "the next asks");
}

#[test]
fn escape_gives_up_each_queued_file_in_turn() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let (viewer, cx, paths) = asked(&mut cx, dir.path());
    assert_eq!(cx.update(|_, app| asking(app, &viewer)), Some(paths[0].clone()));
    cx.simulate_keystrokes("escape");
    settle(cx, |app| asking(app, &viewer) == Some(paths[1].clone()));
    frames(cx);
    cx.simulate_keystrokes("escape");
    settle(cx, |app| viewer.read(app).core.dialog.is_none());
    assert!(cx.update(|_, app| viewer.read(app).active_tab().is_none()));
}

/// Two zoomed Letter pages side by side.
fn zoomed_pair<'a>(cx: &'a mut TestAppContext, dir: &std::path::Path) -> (Entity<PdfView>, &'a mut VisualTestContext) {
    let pages: &[(f32, f32, &[&str])] = &[(612.0, 792.0, &["one"]), (612.0, 792.0, &["two"])];
    let (viewer, cx) = windowed(cx, dir, &[file(dir, "pair.pdf", pages)], true);
    let view = shown(cx, &viewer);
    settle(cx, |app| view.read(app).room.size.width > px(100.0));
    cx.update(|_, app| view.update(app, |view, cx| {
        (view.mode, view.zoom) = (Mode::Two, Zoom::Scale(4.0));
        cx.notify();
    }));
    frames(cx);
    (view, cx)
}

#[test]
fn two_pages_keep_the_right_hand_page() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let pages: &[(f32, f32, &[&str])] = &[(612.0, 792.0, &["one"]), (612.0, 792.0, &["two"]), (612.0, 792.0, &["three"])];
    let (viewer, cx) = windowed(&mut cx, dir.path(), &[file(dir.path(), "three.pdf", pages)], true);
    let view = shown(cx, &viewer);
    settle(cx, |app| view.read(app).room.size.width > px(100.0));
    cx.update(|_, app| view.update(app, |view, cx| {
        view.mode = Mode::Two;
        view.show(1, 0.0, cx);
    }));
    frames(cx);
    assert_eq!(cx.update(|_, app| view.read(app).current), 1);
}

#[test]
fn scrolling_across_to_the_right_page_makes_it_current() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let (view, cx) = zoomed_pair(&mut cx, dir.path());
    cx.update(|_, app| view.update(app, |view, cx| {
        let right = view.laid_out().placed[1].x;
        let y = view.scroll.offset().y;
        view.scroll.set_offset(point(px(-right), y));
        cx.notify();
    }));
    frames(cx);
    assert_eq!(cx.update(|_, app| view.read(app).current), 1);
}

#[test]
fn a_jump_to_the_right_page_brings_it_across() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let (view, cx) = zoomed_pair(&mut cx, dir.path());
    cx.update(|_, app| view.update(app, |view, cx| view.show(1, 0.0, cx)));
    frames(cx);
    let (x, left, wide) = cx.update(|_, app| {
        let view = view.read(app);
        (view.laid_out().placed[1].x, -f32::from(view.scroll.offset().x), f32::from(view.room.size.width))
    });
    assert!(x >= left && x < left + wide, "page at {x}, view {left}..{}", left + wide);
}
