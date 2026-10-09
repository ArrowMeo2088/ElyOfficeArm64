use super::*;

#[test]
fn a_fitted_slide_fills_the_viewport_centred() {
    let Some(install) = crate::client::tests::install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let cx = gpui::TestAppContext::single();
    cx.update(|app| {
        let deck = crate::client::tests::show::DECK;
        let (view, _) = live_view_of(app, &install, "deck.fodp", deck);
        view.update(app, |view, cx| {
            view.fit_page(cx);
            let bounds = Bounds {
                origin: point(px(10.0), px(20.0)),
                size: size(px(800.0), px(600.0)),
            };
            view.prepare(bounds);
            let (w, h) = view.doc_size;
            let want = (800.0 * 15.0 / w as f32).min(600.0 * 15.0 / h as f32) * 0.94;
            assert!(
                (view.geom.zoom - want).abs() < 0.001,
                "{} vs {want}",
                view.geom.zoom
            );
            assert!(
                view.spare.0 > 0.0 && view.spare.1 > 0.0,
                "centred: {:?}",
                view.spare
            );
            let corner = view.window_point((0, 0));
            assert_eq!(view.doc_point(corner), (0, 0), "points map both ways");
            // A cursor far out must not move a fitted page.
            view.reach = (w * 3, h * 3);
            view.prepare(bounds);
            assert_eq!(view.window_point((0, 0)), corner, "the page stays put");
            let far = view.window_point((w, h));
            assert!(
                f32::from(far.y) < 620.0 && f32::from(far.x) < 810.0,
                "the page fits"
            );
        });
    });
}

#[test]
fn a_resized_slide_reaches_the_view_and_still_fits() {
    let Some(install) = crate::client::tests::install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let cx = gpui::TestAppContext::single();
    let view = cx.update(|app| {
        let deck = crate::client::tests::show::DECK;
        live_view_of(app, &install, "deck.fodp", deck).0
    });
    cx.update(|app| {
        view.update(app, |view, cx| {
            view.fit_page(cx);
            view.run("slide.size", &["300000", "300000", "true"])
                .expect("sent");
        })
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while cx.update(|app| view.read(app).doc_size.0 < 100_000) {
        assert!(
            std::time::Instant::now() < deadline,
            "the view never read the new size"
        );
        if !cx.executor().tick() {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    cx.update(|app| {
        view.update(app, |view, _| {
            let bounds = Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(800.0), px(600.0)),
            };
            view.prepare(bounds);
            let (w, h) = view.doc_size;
            let far = view.window_point((w, h));
            assert!(
                f32::from(far.x) <= 800.0 && f32::from(far.y) <= 600.0,
                "a 3 m page fits: {far:?}"
            );
        })
    });
}
