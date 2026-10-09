use super::*;
use gpui::TestAppContext;

/// Steps the scheduler until `done` or ten seconds pass.
fn settle(cx: &TestAppContext, done: impl Fn(&App) -> bool) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        if !cx.executor().tick() {
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(20));
        }
        if cx.update(|app| done(app)) {
            return true;
        }
    }
    false
}

/// The view's 800 by 600 pixel area.
fn area() -> Bounds<Pixels> {
    Bounds {
        origin: point(px(0.0), px(0.0)),
        size: size(px(800.0), px(600.0)),
    }
}

#[test]
fn the_view_paints_the_app_theme_and_back() {
    let Some(install) = crate::client::tests::install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let cx = TestAppContext::single();
    let (view, worker) = cx.update(|app| live_view(app, &install));
    let doc = cx.update(|app| view.read(app).doc.clone());
    let mut shades = Vec::new();
    for dark in [true, false] {
        cx.update(|app| view.update(app, |view, _| view.follow_theme(dark)));
        assert!(
            settle(&cx, |app| view.read(app).settled()),
            "the theme goes out"
        );
        let mut worker = worker.lock().expect("the worker");
        let tile = doc
            .paint_tile(&mut worker, (1440, 1440, 3840, 3840), (64, 64))
            .expect("a tile");
        let sum: u64 = tile
            .pixels
            .chunks_exact(4)
            .map(|px| u64::from(px[0]) + u64::from(px[1]) + u64::from(px[2]))
            .sum();
        shades.push(sum / (64 * 64 * 3));
    }
    assert!(
        shades[0] < 100 && shades[1] > 200,
        "dark then light: {shades:?}"
    );
}

#[test]
fn a_failed_tile_reports_and_retries_on_a_move() {
    let Some(install) = crate::client::tests::install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let cx = TestAppContext::single();
    let (view, _) = cx.update(|app| live_view(app, &install));
    let errors = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    cx.update(|app| {
        let errors = errors.clone();
        app.subscribe(&view, move |_, error: &LokError, _| {
            errors.borrow_mut().push(error.0.clone())
        })
        .detach();
        view.update(app, |view, cx| {
            let geom = view.geom;
            view.prepare(area());
            view.fetching.remove(&(0, 0));
            view.finish_tile((0, 0), geom, Err(io::Error::other("no paint")), cx);
            assert!(view.failed.contains(&((0, 0), view.generation)));
            view.prepare(area());
            assert!(!view.fetching.contains(&(0, 0)), "no retry storm in place");
            view.scroll_by(0.0, -40.0, cx);
            view.prepare(area());
            assert!(view.fetching.contains(&(0, 0)), "a move retries the tile");
        });
    });
    assert!(
        settle(&cx, |_| !errors.borrow().is_empty()),
        "the failure reaches the app"
    );
    assert!(
        errors.borrow()[0].contains("no paint"),
        "{:?}",
        errors.borrow()
    );
}
