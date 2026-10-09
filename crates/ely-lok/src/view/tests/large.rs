use super::*;

/// A sheet whose last value sits 100,000 rows down.
const TALL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
 xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
 xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" office:version="1.3" office:mimetype="application/vnd.oasis.opendocument.spreadsheet">
 <office:body><office:spreadsheet><table:table table:name="S">
 <table:table-row><table:table-cell><text:p>top</text:p></table:table-cell></table:table-row>
 <table:table-row table:number-rows-repeated="99998"><table:table-cell/></table:table-row>
 <table:table-row><table:table-cell><text:p>bottom</text:p></table:table-cell></table:table-row>
 </table:table></office:spreadsheet></office:body>
</office:document>"#;

#[test]
fn a_tall_sheet_paints_and_keeps_only_tiles_near_the_screen() {
    let Some(install) = crate::client::tests::install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let cx = gpui::TestAppContext::single();
    cx.update(|app| {
        let (view, _) = live_view_of(app, &install, "tall.fods", TALL);
        view.update(app, |view, cx| {
            let bounds = Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(800.0), px(600.0)),
            };
            let side = view.geom.tile_pixels() as usize;
            let painted = |generation| {
                Ok(crate::Tile {
                    generation,
                    pixels: vec![0u8; side * side * 4],
                })
            };
            // 800 by 600 needs 5 by 4 tiles.
            view.prepare(bounds);
            let top = view.on_screen.clone();
            assert!(!top.is_empty() && top.len() <= 20, "{} tiles", top.len());
            for key in &top {
                view.finish_tile(*key, view.geom, painted(view.generation), cx);
            }
            assert_eq!(view.tile_count(), top.len(), "the top paints");
            view.scroll_by(0.0, -3.0e6, cx);
            view.prepare(bounds);
            let bottom = view.on_screen.clone();
            assert!(bottom.len() <= 20, "{} tiles at the bottom", bottom.len());
            assert!(
                bottom.iter().all(|key| key.1 > 1000),
                "far down: {bottom:?}"
            );
            assert!(
                top.iter().all(|key| !view.tiles.contains_key(key)),
                "the top's tiles are gone"
            );
            assert!(
                view.fetching.len() <= 20 + top.len(),
                "only shown tiles fetch"
            );
        });
    });
}

/// 6000 rows of 0.5 cm; tiles round each to pixels.
const SHORT_ROWS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
 xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
 xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
 xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" office:version="1.3" office:mimetype="application/vnd.oasis.opendocument.spreadsheet">
 <office:automatic-styles><style:style style:name="ro1" style:family="table-row"><style:table-row-properties style:row-height="0.5cm" style:use-optimal-row-height="false"/></style:style></office:automatic-styles>
 <office:body><office:spreadsheet><table:table table:name="S">
 <table:table-row table:style-name="ro1" table:number-rows-repeated="6000"><table:table-cell table:number-columns-repeated="4"/></table:table-row>
 </table:table></office:spreadsheet></office:body>
</office:document>"#;

/// A short-row sheet at 2× DPI, scrolled to `cell`'s row.
fn far_down(cx: &mut gpui::TestAppContext, install: &std::path::Path) -> (Entity<LokView>, crate::sheet::Geometry) {
    let (view, worker) = cx.update(|app| live_view_of(app, install, "short.fods", SHORT_ROWS));
    let json = cx.update(|app| {
        let read = view.read(app);
        worker.lock().expect("the worker lock").values(&read.doc, crate::sheet::GEOMETRY).expect("geometry")
    });
    let geometry = crate::sheet::geometry(&json).expect("parsed");
    let row = cell(&geometry, 1, 4997);
    cx.update(|app| {
        view.update(app, |view, cx| {
            view.geom.dpi = 2.0;
            view.set_geometry(geometry.clone(), cx);
            view.prepare(Bounds { origin: point(px(0.0), px(0.0)), size: size(px(800.0), px(600.0)) });
            view.show(row, cx);
        })
    });
    (view, geometry)
}

/// Column `x`, row `y` in print twips.
fn cell(geometry: &crate::sheet::Geometry, x: u32, y: u32) -> crate::Rect {
    (geometry.columns.start(x), geometry.rows.start(y), geometry.columns.size(x), geometry.rows.size(y))
}

/// Waits until the kernel reports `want` as the selection.
fn selected(view: &Entity<LokView>, cx: &mut gpui::TestAppContext, want: &str) -> String {
    let mut address = String::new();
    for _ in 0..200 {
        cx.executor().tick();
        address = cx.update(|app| view.read(app).cell().address.clone());
        if address == want {
            break;
        }
    }
    address
}

#[test]
fn a_click_far_down_a_sheet_selects_the_cell_under_it() {
    let Some(install) = crate::client::tests::install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let mut cx = gpui::TestAppContext::single();
    let (view, geometry) = far_down(&mut cx, &install);
    let (x, y, w, h) = cell(&geometry, 1, 4997);
    cx.update(|app| {
        view.update(app, |view, _| {
            let at = view.window_point((x + w / 2, y + h / 2));
            for kind in [0, 1] {
                view.mouse(kind, at, 1, 1, &gpui::Modifiers::default());
            }
        })
    });
    assert_eq!(selected(&view, &mut cx, "B4998"), "B4998");
}

#[test]
fn a_drag_far_down_a_sheet_selects_the_range_under_it() {
    let Some(install) = crate::client::tests::install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let mut cx = gpui::TestAppContext::single();
    let (view, geometry) = far_down(&mut cx, &install);
    let middle = |(x, y, w, h): crate::Rect| (x + w / 2, y + h / 2);
    let (from, to) = (middle(cell(&geometry, 1, 4997)), middle(cell(&geometry, 3, 5000)));
    cx.update(|app| {
        view.update(app, |view, _| {
            let modifiers = gpui::Modifiers::default();
            view.mouse(0, view.window_point(from), 1, 1, &modifiers);
            view.mouse(2, view.window_point(((from.0 + to.0) / 2, (from.1 + to.1) / 2)), 1, 1, &modifiers);
            view.mouse(2, view.window_point(to), 1, 1, &modifiers);
            view.mouse(1, view.window_point(to), 1, 1, &modifiers);
        })
    });
    assert_eq!(selected(&view, &mut cx, "B4998:D5001"), "B4998:D5001");
}

#[test]
fn zooming_far_down_a_sheet_keeps_the_rows_in_view() {
    let Some(install) = crate::client::tests::install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let mut cx = gpui::TestAppContext::single();
    let (view, geometry) = far_down(&mut cx, &install);
    let middle = |cx: &mut gpui::TestAppContext| {
        cx.update(|app| {
            let read = view.read(app);
            let (w, h) = read.viewport();
            let at = read.origin() + point(px(w / 2.0), px(h / 2.0));
            geometry.rows.at(read.doc_point(at).1)
        })
    };
    let before = middle(&mut cx);
    cx.update(|app| view.update(app, |view, cx| view.set_zoom(0.8, cx)));
    let after = middle(&mut cx);
    assert!(before.abs_diff(after) <= 1, "row {before} became {after}");
}
