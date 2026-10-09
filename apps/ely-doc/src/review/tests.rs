use gpui::{Entity, TestAppContext};

use super::*;
use crate::actions::Dialog;
use crate::refs::InsertComment;
use crate::tests::{blank, docs, settle, settled, text_of, type_into, with_window, written};
use ely_office::prelude::*;

/// The review panel on the active tab.
fn panel(cx: &mut TestAppContext, docs: &Entity<Docs>) -> Entity<Review> {
    cx.update(|app| {
        let tab = docs.read(app).active_tab().expect("a tab");
        let (view, doc) = (tab.view.clone(), tab.doc.clone());
        let review = docs.read(app).review.clone();
        review.update(app, |review, cx| review.show(Some((&view, &doc)), cx));
        review
    })
}

#[test]
fn comments_thread_and_changes_accept_or_reject() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let Some(docs) = docs(&mut cx, dir.path()) else {
        eprintln!("SKIP: no LOK build");
        return;
    };
    let id = blank(&mut cx, &docs);
    type_into(&mut cx, &docs, "Hello");
    let review = panel(&mut cx, &docs);
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.on_comment(&InsertComment, window, cx);
        let Some(Dialog::Ask { ask, .. }) = &docs.dialog else {
            panic!("the comment prompt is open");
        };
        let ask = ask.clone();
        docs.answer(&ask, "First note", cx);
    });
    assert_eq!(
        cx.update(|app| docs.read(app).side),
        Side::Review,
        "the panel shows"
    );
    settle(&cx, |app| review.read(app).threads.len() == 1);
    let key = cx.update(|app| review.read(app).threads[0].key.clone());
    cx.update(|app| review.update(app, |review, cx| review.select(&key, cx)));
    cx.update(|app| review.update(app, |review, cx| review.reply("An answer", cx)));
    settle(&cx, |app| {
        review
            .read(app)
            .threads
            .first()
            .is_some_and(|thread| thread.comments.len() == 2)
    });
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.on_track(&TrackChanges, window, cx)
    });
    settle(&cx, |app| {
        let tab = docs.read(app).active_tab().expect("a tab");
        tab.view.read(app).state(".uno:TrackChanges") == Some("true")
    });
    type_into(&mut cx, &docs, " kept");
    settle(&cx, |app| review.read(app).changes.len() == 1);
    let index = cx.update(|app| review.read(app).changes[0].index.to_string());
    cx.update(|app| review.update(app, |review, cx| review.run("track.accept", &[&index], cx)));
    settle(&cx, |app| review.read(app).changes.is_empty());
    type_into(&mut cx, &docs, " gone");
    settle(&cx, |app| review.read(app).changes.len() == 1);
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.on_reject_all(&RejectAll, window, cx)
    });
    settle(&cx, |app| review.read(app).changes.is_empty());
    settled(&cx, &docs);
    let path = dir.path().join("review.odt");
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.write_as(id, path.clone(), None, window, cx)
    });
    written(&cx, &path, None);
    let text = text_of(&mut cx, &docs, &path);
    for mark in ["First note", "An answer", " kept"] {
        assert!(text.contains(mark), "{mark} lands");
    }
    assert!(!text.contains("gone"), "the rejected words go");
}
