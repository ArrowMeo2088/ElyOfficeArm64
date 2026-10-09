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
