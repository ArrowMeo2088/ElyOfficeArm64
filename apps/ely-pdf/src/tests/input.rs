//! Mouse, jumps, search across tabs, failures, tiles (C11.1).

use std::collections::HashSet;

use ely_office::Tabs as _;
use gpui::{Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px};

use super::{file, settle, shown, windowed};
use crate::layout::Zoom;
use crate::view::PdfView;
use crate::viewer::Viewer;

/// The window point over page points `at` of `page`.
pub(super) fn over(cx: &mut VisualTestContext, view: &Entity<PdfView>, page: usize, at: (f32, f32)) -> gpui::Point<gpui::Pixels> {
    cx.update(|_, app| {
        let view = view.read(app);
        let laid = view.laid_out();
        let place = laid.placed.iter().find(|p| p.page == page).expect("placed");
        let (offset, origin) = (view.scroll.offset(), view.room.origin);
        point(
            origin.x + offset.x + px(place.x + at.0 * laid.scale),
            origin.y + offset.y + px(place.y + at.1 * laid.scale),
        )
    })
}

#[test]
fn a_first_drag_picks_text_before_the_page_is_read() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let path = file(dir.path(), "one.pdf", &[(612.0, 792.0, &["alpha beta gamma"])]);
    let (viewer, cx) = windowed(&mut cx, dir.path(), &[path], true);
    let view = shown(cx, &viewer);
    settle(cx, |app| view.read(app).room.size.width > px(100.0));
    // Glyphs sit near 72 points down; none read yet.
    let (from, to) = (over(cx, &view, 0, (74.0, 62.0)), over(cx, &view, 0, (150.0, 62.0)));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    cx.update(|_, app| view.update(app, |view, cx| view.copy(cx)));
    settle(cx, |app| app.read_from_clipboard().is_some());
    let text = cx.update(|_, app| app.read_from_clipboard().and_then(|item| item.text())).expect("copied");
    assert!(text.starts_with("alpha"), "{text}");
}

#[test]
fn a_jump_makes_its_page_the_one_being_read() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let pages: &[(f32, f32, &[&str])] = &[(612.0, 792.0, &["one"]), (612.0, 792.0, &["two"]), (612.0, 792.0, &["three"])];
    let (viewer, cx) = windowed(&mut cx, dir.path(), &[file(dir.path(), "three.pdf", pages)], true);
    let view = shown(cx, &viewer);
    settle(cx, |app| view.read(app).room.size.width > px(100.0));
    cx.update(|_, app| view.update(app, |view, cx| view.show(1, 0.0, cx)));
    super::frames(cx);
    assert_eq!(cx.update(|_, app| view.read(app).current), 1);
}

/// A: the word on page three only. B: three pages.
fn two_tabs<'a>(cx: &'a mut TestAppContext, dir: &std::path::Path) -> (Entity<Viewer>, &'a mut VisualTestContext, u64, u64) {
    let a = file(dir, "a.pdf", &[(612.0, 792.0, &["one"]), (612.0, 792.0, &["two"]), (612.0, 792.0, &["three alpha"])]);
    let b = file(dir, "b.pdf", &[(612.0, 792.0, &["zzz"]), (612.0, 792.0, &["b2"]), (612.0, 792.0, &["b3"])]);
    let (viewer, cx) = windowed(cx, dir, &[a, b], true);
    let (first, second) = cx.update(|_, app| (viewer.read(app).core.tabs[0].id, viewer.read(app).core.tabs[1].id));
    (viewer, cx, first, second)
}

/// Opens find on `tab` and searches `query` at once.
fn find_in(cx: &mut VisualTestContext, viewer: &Entity<Viewer>, tab: u64, query: &str) {
    cx.update(|window, app| viewer.update(app, |viewer, cx| {
        viewer.core.active = Some(tab);
        viewer.open_find(window, cx);
        let field = viewer.find.as_ref().expect("open").query.clone();
        field.update(cx, |field, cx| field.set_text(query, cx));
    }));
}

fn view_of(cx: &mut VisualTestContext, viewer: &Entity<Viewer>, tab: u64) -> Entity<PdfView> {
    cx.update(|_, app| viewer.read(app).tab(tab).expect("open").view.clone())
}

#[test]
fn a_hit_found_late_never_moves_another_tab() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let (viewer, cx, first, second) = two_tabs(&mut cx, dir.path());
    find_in(cx, &viewer, first, "alpha");
    // Before the batch comes back, the reader turns to B.
    cx.update(|_, app| viewer.update(app, |viewer, _| viewer.core.active = Some(second)));
    settle(cx, |app| viewer.read(app).find.as_ref().is_some_and(|find| find.done == 3));
    let (a, b) = (view_of(cx, &viewer, first), view_of(cx, &viewer, second));
    let state = cx.update(|_, app| (a.read(app).chosen, b.read(app).current));
    assert_eq!(state, (Some(0), 0), "the hit is chosen; B stays on its page");
}

#[test]
fn a_new_find_box_takes_no_answers_meant_for_the_old() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let (viewer, cx, first, second) = two_tabs(&mut cx, dir.path());
    find_in(cx, &viewer, first, "alpha");
    find_in(cx, &viewer, second, "zzz");
    settle(cx, |app| viewer.read(app).find.as_ref().is_some_and(|find| find.done == 3));
    super::frames(cx);
    let (a, b) = (view_of(cx, &viewer, first), view_of(cx, &viewer, second));
    let pages = cx.update(|_, app| (a.read(app).found.len(), b.read(app).found.iter().map(|h| h.0).collect::<Vec<_>>()));
    assert_eq!(pages, (0, vec![0]), "only B's own hit");
}

#[test]
fn a_tile_that_fails_is_not_asked_again() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let (viewer, cx) = windowed(&mut cx, dir.path(), &[file(dir.path(), "one.pdf", &[(612.0, 792.0, &["x"])])], true);
    let view = shown(cx, &viewer);
    // The document goes away behind the viewer's back.
    let (worker, doc) = cx.update(|_, app| (viewer.read(app).core.worker.clone(), view.read(app).doc));
    worker.lock().expect("the lock").get().expect("started").close(doc).expect("closes");
    cx.update(|_, app| view.update(app, |view, cx| {
        view.tiles.clear();
        cx.notify();
    }));
    settle(cx, |app| !view.read(app).failed.is_empty());
    // Bounded steps: a retry loop would never go quiet.
    for _ in 0..10 {
        super::frames(cx);
    }
    assert!(!cx.update(|_, app| view.read(app).painting), "no new tries, nothing waiting");
}

#[test]
fn tiles_scrolled_off_a_long_page_go() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let (viewer, cx) = windowed(&mut cx, dir.path(), &[file(dir.path(), "long.pdf", &[(612.0, 14400.0, &["top"])])], true);
    let view = shown(cx, &viewer);
    cx.update(|_, app| view.update(app, |view, cx| {
        view.zoom = Zoom::Scale(1.0);
        cx.notify();
    }));
    settle(cx, |app| view.read(app).tiles.keys().any(|key| key.ty == 0));
    cx.update(|_, app| view.update(app, |view, cx| {
        view.go_to = Some((0.0, 4000.0));
        cx.notify();
    }));
    settle(cx, |app| !view.read(app).tiles.is_empty() && view.read(app).tiles.keys().all(|key| key.ty > 0));
}

#[test]
fn no_worker_runs_before_a_file_opens() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let (viewer, cx) = windowed(&mut cx, dir.path(), &[], false);
    let started = |cx: &mut VisualTestContext| cx.update(|_, app| viewer.read(app).core.worker.lock().expect("the lock").started());
    assert!(!started(cx));
    let path = file(dir.path(), "plain.pdf", &[(612.0, 792.0, &["no marks"])]);
    cx.update(|_, app| viewer.update(app, |viewer, cx| ely_office::Opening::open_file(viewer, path.clone(), cx)));
    settle(cx, |app| viewer.read(app).active_tab().is_some());
    assert!(started(cx));
}

#[test]
fn tiles_beside_the_view_are_neither_asked_nor_kept() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let (viewer, cx) = windowed(&mut cx, dir.path(), &[file(dir.path(), "wide.pdf", &[(612.0, 792.0, &["x"])])], true);
    let view = shown(cx, &viewer);
    settle(cx, |app| view.read(app).room.size.width > px(100.0));
    cx.update(|_, app| view.update(app, |view, cx| {
        view.zoom = Zoom::Scale(4.0);
        cx.notify();
    }));
    let columns = |app: &gpui::App| view.read(app).tiles.keys().map(|key| key.tx).collect::<HashSet<_>>();
    let last = cx.update(|window, app| {
        let laid = view.read(app).laid_out();
        ((laid.placed[0].w * window.scale_factor()).ceil() as i32 - 1) / crate::pages::TILE
    });
    settle(cx, |app| columns(app).contains(&0) && !view.read(app).painting);
    assert!(!cx.update(|_, app| columns(app)).contains(&last), "the right edge is off screen");
    cx.update(|_, app| view.update(app, |view, cx| {
        let wide = view.laid_out().size.0 - f32::from(view.room.size.width);
        let y = view.scroll.offset().y;
        view.scroll.set_offset(point(px(-wide), y));
        cx.notify();
    }));
    settle(cx, |app| columns(app).contains(&last) && !columns(app).contains(&0));
}

#[test]
fn an_edit_keeps_the_page_drawn_until_its_repaint_lands() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let (viewer, cx) = windowed(&mut cx, dir.path(), &[file(dir.path(), "one.pdf", &[(612.0, 792.0, &["Hello"])])], true);
    let view = shown(cx, &viewer);
    settle(cx, |app| !view.read(app).tiles.is_empty() && !view.read(app).painting);
    let before: Vec<_> = cx.update(|_, app| view.read(app).tiles.values().cloned().collect());
    let note = ely_pdf_engine::NewAnnot {
        page: 0,
        shape: ely_pdf_engine::Shape::Note((100.0, 500.0)),
        rgb: None,
        contents: "Look".into(),
        author: String::new(),
    };
    // Watches every notify, the edit's own included, before repaints.
    let blank = std::rc::Rc::new(std::cell::Cell::new(false));
    let seen = blank.clone();
    let _watch = cx.update(|_, app| {
        app.observe(&view, move |view, app| {
            let read = view.read(app);
            if read.edited == 1 && read.tiles.is_empty() {
                seen.set(true);
            }
        })
    });
    cx.update(|_, app| view.update(app, |view, cx| view.annotate("adding a note", vec![note], cx)));
    settle(cx, |app| {
        let read = view.read(app);
        read.edited == 1 && read.stale.is_empty() && !read.painting
    });
    assert!(!blank.get(), "the page never goes blank");
    let after = cx.update(|_, app| view.read(app).tiles.values().cloned().collect::<Vec<_>>());
    assert!(
        after.iter().all(|image| before.iter().all(|old| !std::sync::Arc::ptr_eq(old, image))),
        "every tile was painted again"
    );
}
