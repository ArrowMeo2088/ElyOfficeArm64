use gpui::{Entity, TestAppContext};

use super::*;
use crate::tests::{docs, settle, settled, text_of, with_window, written};
use ely_office::Dialog;

/// Three pages, two headings, a font the system lacks.
const LONG: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" office:version="1.3" office:mimetype="application/vnd.oasis.opendocument.text">
 <office:font-face-decls><style:font-face style:name="Nowhere Sans" svg:font-family="&apos;Nowhere Sans&apos;"/></office:font-face-decls>
 <office:automatic-styles>
  <style:style style:name="P1" style:family="paragraph"><style:paragraph-properties fo:break-before="page"/></style:style>
  <style:style style:name="T1" style:family="text"><style:text-properties style:font-name="Nowhere Sans"/></style:style>
 </office:automatic-styles>
 <office:body><office:text>
  <text:h text:outline-level="1">First part</text:h><text:p>alpha <text:span text:style-name="T1">odd</text:span></text:p>
  <text:h text:style-name="P1" text:outline-level="1">Second part</text:h><text:p>bravo</text:p>
  <text:p text:style-name="P1">charlie</text:p>
 </office:text></office:body>
</office:document>"#;

/// Two headings side by side in two columns.
const COLUMNS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" office:version="1.3" office:mimetype="application/vnd.oasis.opendocument.text">
 <office:automatic-styles>
  <style:style style:name="P1" style:family="paragraph"><style:paragraph-properties fo:break-before="column"/></style:style>
  <style:page-layout style:name="pm1"><style:page-layout-properties><style:columns fo:column-count="2" fo:column-gap="0.5cm"/></style:page-layout-properties></style:page-layout>
 </office:automatic-styles>
 <office:master-styles><style:master-page style:name="Standard" style:page-layout-name="pm1"/></office:master-styles>
 <office:body><office:text>
  <text:h text:outline-level="1">First part</text:h><text:p>alpha</text:p>
  <text:h text:style-name="P1" text:outline-level="1">Second part</text:h><text:p>bravo</text:p>
 </office:text></office:body>
</office:document>"#;

fn open_long(cx: &mut TestAppContext, docs: &Entity<Docs>, dir: &std::path::Path) -> u64 {
    open_text(cx, docs, &dir.join("long.fodt"), LONG)
}

fn open_text(
    cx: &mut TestAppContext,
    docs: &Entity<Docs>,
    path: &std::path::Path,
    text: &str,
) -> u64 {
    let path = path.to_path_buf();
    std::fs::write(&path, text).expect("the file writes");
    with_window(cx, docs, |docs, _, cx| docs.open_file(path.clone(), cx));
    settle(cx, |app| docs.read(app).core.tabs.len() == 1);
    settled(cx, docs);
    cx.update(|app| docs.read(app).core.tabs[0].id)
}

fn cursor_y(app: &gpui::App, docs: &Entity<Docs>) -> Option<i32> {
    let tab = docs.read(app).active_tab()?;
    tab.view.read(app).cursor().map(|cursor| cursor.1)
}

#[test]
fn headings_and_pages_jump_and_back_returns() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let Some(docs) = docs(&mut cx, dir.path()) else {
        eprintln!("SKIP: no LOK build");
        return;
    };
    open_long(&mut cx, &docs, dir.path());
    settle(&cx, |app| {
        let tab = docs.read(app).active_tab().expect("a tab");
        tab.view.read(app).missing_fonts() == ["Nowhere Sans"]
            && tab.view.read(app).cursor().is_some()
    });
    let navigator = cx.update(|app| docs.read(app).navigator.clone());
    cx.update(|app| {
        let (view, doc) = {
            let tab = docs.read(app).active_tab().expect("a tab");
            (tab.view.clone(), tab.doc.clone())
        };
        navigator.update(app, |navigator, cx| navigator.show(Some((&view, &doc)), cx));
    });
    settle(&cx, |app| navigator.read(app).outline.len() == 2);
    let current = |app: &gpui::App| {
        let navigator = navigator.read(app);
        navigator.outline.iter().position(|heading| heading.current)
    };
    settle(&cx, |app| current(app) == Some(0));
    let start = cx.update(|app| cursor_y(app, &docs)).expect("a cursor");
    let (first, second) = cx.update(|app| {
        let outline = &navigator.read(app).outline;
        (outline[0].rect, outline[1].rect)
    });
    let third = cx.update(|app| {
        let tab = docs.read(app).active_tab().expect("a tab");
        tab.view.read(app).pages()[2]
    });
    cx.update(|app| {
        docs.update(app, |docs, cx| {
            docs.jump(second, cx);
            docs.jump((third.0, third.1, 0, 0), cx);
        })
    });
    settle(&cx, |app| {
        cursor_y(app, &docs).is_some_and(|y| y >= third.1)
    });
    assert_eq!(
        cx.update(|app| docs.read(app).back.last().map(|place| place.1)),
        Some(second),
        "a jump in flight is the place"
    );
    settle(&cx, |app| current(app) == Some(1));
    let between =
        |app: &gpui::App| cursor_y(app, &docs).is_some_and(|y| y >= second.1 && y < third.1);
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.on_back(&GoBack, window, cx)
    });
    settle(&cx, between);
    cx.update(|app| docs.update(app, |docs, cx| docs.jump(first, cx)));
    settle(&cx, |app| current(app) == Some(0));
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.on_back(&GoBack, window, cx)
    });
    settle(&cx, between);
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.on_back(&GoBack, window, cx)
    });
    settle(&cx, |app| {
        cursor_y(app, &docs).is_some_and(|y| y <= start + 300)
    });
    let home = cx.update(|app| {
        let tab = docs.read(app).active_tab().expect("a tab");
        tab.view.read(app).cursor().expect("a cursor")
    });
    cx.update(|app| docs.update(app, |docs, cx| docs.jump(second, cx)));
    settle(&cx, between);
    let view = cx.update(|app| docs.read(app).active_tab().expect("a tab").view.clone());
    cx.update(|app| view.update(app, |view, cx| view.reveal(home, cx)));
    settle(&cx, |app| {
        cursor_y(app, &docs).is_some_and(|y| y <= start + 300)
    });
    cx.update(|app| docs.update(app, |docs, cx| docs.jump((third.0, third.1, 0, 0), cx)));
    settle(&cx, |app| {
        cursor_y(app, &docs).is_some_and(|y| y >= third.1)
    });
    assert_eq!(
        cx.update(|app| docs.read(app).back.last().map(|place| place.1)),
        Some(home),
        "a landed jump is no longer the place"
    );
}

#[test]
fn the_current_heading_follows_the_cursor_across_columns() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let Some(docs) = docs(&mut cx, dir.path()) else {
        eprintln!("SKIP: no LOK build");
        return;
    };
    open_text(&mut cx, &docs, &dir.path().join("columns.fodt"), COLUMNS);
    let navigator = cx.update(|app| docs.read(app).navigator.clone());
    cx.update(|app| {
        let (view, doc) = {
            let tab = docs.read(app).active_tab().expect("a tab");
            (tab.view.clone(), tab.doc.clone())
        };
        navigator.update(app, |navigator, cx| navigator.show(Some((&view, &doc)), cx));
    });
    let current = |app: &gpui::App| {
        let navigator = navigator.read(app);
        navigator.outline.iter().position(|heading| heading.current)
    };
    settle(&cx, |app| current(app) == Some(0));
    let (first, second) = cx.update(|app| {
        let outline = &navigator.read(app).outline;
        (outline[0].rect, outline[1].rect)
    });
    assert_eq!(first.1, second.1, "the headings sit side by side");
    cx.update(|app| docs.update(app, |docs, cx| docs.jump(second, cx)));
    settle(&cx, |app| current(app) == Some(1));
    cx.update(|app| docs.update(app, |docs, cx| docs.jump(first, cx)));
    settle(&cx, |app| current(app) == Some(0));
}

#[cfg(unix)]
#[test]
fn pdfs_export_a_range_and_locked_files_ask() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let Some(docs) = docs(&mut cx, dir.path()) else {
        eprintln!("SKIP: no LOK build");
        return;
    };
    let id = open_long(&mut cx, &docs, dir.path());
    let pdf = dir.path().join("two.pdf");
    let only_two = ely_lok::Pdf {
        pages: Some((2, 2)),
        comments: false,
        changes: true,
        handouts: 0,
    };
    cx.update(|app| {
        docs.update(app, |docs, cx| {
            docs.export_to(id, pdf.clone(), only_two, cx)
        })
    });
    written(&cx, &pdf, None);
    let source = dir.path().join("long.fodt");
    let alias = dir.path().join("alias.pdf");
    std::os::unix::fs::symlink(&source, &alias).expect("the alias");
    let link = dir.path().join("link.pdf");
    std::fs::hard_link(&source, &link).expect("the hard link");
    for target in [&source, &alias, &link] {
        cx.update(|app| {
            docs.update(app, |docs, cx| {
                docs.export_to(id, target.clone(), ely_lok::Pdf::default(), cx)
            })
        });
    }
    assert_eq!(
        std::fs::read_to_string(&source).expect("the source reads"),
        LONG,
        "an open source is never replaced"
    );
    let locked = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/ely-lok/src/client/tests/locked.odt");
    let copy = dir.path().join("locked.odt");
    std::fs::copy(&locked, &copy).expect("the fixture copies");
    let asked = |app: &gpui::App, wrong: bool| matches!(&docs.read(app).core.dialog, Some(Dialog::Password { wrong: was, .. }) if *was == wrong);
    with_window(&mut cx, &docs, |docs, _, cx| {
        docs.open_file(copy.clone(), cx)
    });
    settle(&cx, |app| asked(app, false));
    // As submitting does: the prompt closes, then the load.
    with_window(&mut cx, &docs, |docs, _, cx| {
        docs.core.dialog = None;
        docs.open_with(copy.clone(), "wrong".into(), cx)
    });
    settle(&cx, |app| asked(app, true));
    with_window(&mut cx, &docs, |docs, _, cx| {
        docs.core.dialog = None;
        docs.open_with(copy.clone(), "secret".into(), cx)
    });
    settle(&cx, |app| docs.read(app).core.tabs.len() == 2);
    let out = dir.path().join("opened.odt");
    let opened = cx.update(|app| docs.read(app).core.tabs[1].id);
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.write_as(opened, out.clone(), None, window, cx)
    });
    written(&cx, &out, None);
    assert!(
        text_of(&mut cx, &docs, &out).contains("Teh cat"),
        "the locked text opens"
    );
}
