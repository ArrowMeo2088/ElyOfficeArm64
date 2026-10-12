use gpui::{ScrollDelta, ScrollWheelEvent, TestAppContext, point, px};

use super::Pane;
use crate::desk::tests::{open, start};

#[gpui::test]
fn a_long_outline_scrolls_to_its_last_heading(cx: &mut TestAppContext) {
    let (data, files) = (
        tempfile::tempdir().expect("data"),
        tempfile::tempdir().expect("files"),
    );
    let path = files.path().join("long.md");
    let text: String = (0..100).map(|n| format!("# Heading {n}\n\n")).collect();
    std::fs::write(&path, text).expect("a file");
    let (desk, cx) = start(cx, data.path());
    open(&desk, &path, cx);
    let side = desk.read_with(cx, |desk, _| desk.side.clone());
    side.update(cx, |side, cx| {
        side.pane = Pane::Outline;
        cx.notify();
    });
    cx.run_until_parked();
    let before = cx
        .debug_bounds("mark-outline")
        .expect("the outline is drawn");
    let window = cx.update(|window, _| window.viewport_size());
    assert!(
        before.bottom() > window.height,
        "the outline is longer than the window"
    );
    let at = point(before.left() + px(20.0), px(300.0));
    for _ in 0..20 {
        cx.simulate_event(ScrollWheelEvent {
            position: at,
            delta: ScrollDelta::Pixels(point(px(0.0), px(-1000.0))),
            ..Default::default()
        });
        cx.run_until_parked();
    }
    let after = cx
        .debug_bounds("mark-outline")
        .expect("the outline is drawn");
    assert!(
        after.bottom() <= window.height,
        "the last heading is in view: {after:?}"
    );
}
