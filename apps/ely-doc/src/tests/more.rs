use gpui::TestAppContext;

use super::*;

/// The report template with an embedded Basic library.
fn macro_fodt(cx: &mut TestAppContext) -> String {
    cx.update(|app| Template::Report.fodt(app))
        .replace(
            "office:version=",
            "xmlns:script=\"urn:oasis:names:tc:opendocument:xmlns:script:1.0\" xmlns:ooo=\"http://openoffice.org/2004/office\" office:version=",
        )
        .replace(
            "<office:body>",
            "<office:scripts><office:script script:language=\"ooo:Basic\"><ooo:libraries><ooo:library-embedded ooo:name=\"Standard\"><ooo:module ooo:name=\"Module1\"><ooo:source-code>Sub Main&#x0A;End Sub</ooo:source-code></ooo:module></ooo:library-embedded></ooo:libraries></office:script></office:scripts><office:body>",
        )
}

#[test]
fn zip_macro_checks_read_real_kernel_files() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let Some(docs) = docs(&mut cx, dir.path()) else {
        eprintln!("SKIP: no LOK build");
        return;
    };
    let source = dir.path().join("basic.fodt");
    std::fs::write(&source, macro_fodt(&mut cx)).expect("writes");
    let worker = cx.update(|app| docs.read(app).core.worker.clone());
    let mut worker = worker.lock().expect("the worker lock");
    let doc = worker.load(&ely_lok::file_url(&source)).expect("loads");
    let (odt, docx) = (dir.path().join("basic.odt"), dir.path().join("plain.docx"));
    doc.save_as(&mut worker, &ely_lok::file_url(&odt), "odt")
        .expect("odt");
    doc.save_as(&mut worker, &ely_lok::file_url(&docx), "docx")
        .expect("docx");
    worker.close_doc(doc).expect("closes");
    assert!(
        ely_office::files::macro_risk(&Docs::KIND, &odt).expect("odt"),
        "an ODT with Basic is a risk"
    );
    assert!(
        !ely_office::files::macro_risk(&Docs::KIND, &docx).expect("docx"),
        "a DOCX drops the Basic"
    );
}

#[test]
fn a_local_template_with_macros_opens_in_risk_mode() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let Some(docs) = docs(&mut cx, dir.path()) else {
        eprintln!("SKIP: no LOK build");
        return;
    };
    let template = dir.path().join("macro-template.fodt");
    std::fs::write(&template, macro_fodt(&mut cx)).expect("writes");
    with_window(&mut cx, &docs, |docs, _, cx| {
        docs.new_from_file(&template, cx)
    });
    settle(&cx, |app| docs.read(app).core.active.is_some());
    cx.update(|app| {
        assert!(
            docs.read(app).active_tab().expect("a tab").risk.is_some(),
            "the copy shows the risk"
        )
    });
}

#[test]
fn select_all_alone_leaves_the_tab_clean() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let Some(docs) = docs(&mut cx, dir.path()) else {
        eprintln!("SKIP: no LOK build");
        return;
    };
    blank(&mut cx, &docs);
    let view = cx.update(|app| docs.read(app).active_tab().expect("a tab").view.clone());
    view.update(&mut cx, |view, _| {
        view.run("edit.select_all", &[]).expect("select all queues")
    });
    settle(&cx, |app| view.read(app).settled());
    for _ in 0..40 {
        cx.executor().tick();
    }
    cx.update(|app| {
        assert!(
            !docs.read(app).active_tab().expect("a tab").dirty(app),
            "selection is no edit"
        )
    });
}

#[test]
fn keep_mine_waits_and_two_queued_saves_never_crash() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let Some(docs) = docs(&mut cx, dir.path()) else {
        eprintln!("SKIP: no LOK build");
        return;
    };
    let id = blank(&mut cx, &docs);
    let mine = dir.path().join("mine.docx");
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.write_as(id, mine.clone(), None, window, cx)
    });
    written(&cx, &mine, None);
    type_into(&mut cx, &docs, "x");
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&mine, "EXTERNAL").expect("the outside write");
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.save(id, None, window, cx)
    });
    commit(&mut cx, &docs, "PendingKeepMine");
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.keep_mine(id, Some(After::Tab), window, cx)
    });
    settle(&cx, |app| docs.read(app).core.tabs.is_empty());
    let saved = text_of(&mut cx, &docs, &mine);
    assert!(
        saved.contains("PendingKeepMine"),
        "keep mine saved the queued text: {}",
        saved[saved.find("<office:text").unwrap_or(0)..].replace("\n", " ")
    );

    let id = blank(&mut cx, &docs);
    commit(&mut cx, &docs, "queued");
    let (one, two) = (dir.path().join("one.docx"), dir.path().join("two.docx"));
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.write_as(id, one.clone(), Some(After::Tab), window, cx)
    });
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.write_as(id, two.clone(), None, window, cx)
    });
    settle(&cx, |app| docs.read(app).core.tabs.is_empty());
    for _ in 0..40 {
        cx.executor().tick();
    }
    assert!(
        one.exists() && !two.exists(),
        "the closed tab saves no more"
    );
}

#[test]
fn a_kernel_write_failure_stays_visible() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let Some(docs) = docs(&mut cx, dir.path()) else {
        eprintln!("SKIP: no LOK build");
        return;
    };
    let id = blank(&mut cx, &docs);
    type_into(&mut cx, &docs, "RetainedText");
    let work = cx.update(|app| docs.read(app).active_tab().expect("a tab").work.clone());
    std::fs::remove_dir_all(&work).expect("drops the scratch");
    std::fs::write(&work, b"not a folder").expect("blocks it");
    let target = dir.path().join("failure.docx");
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.write_as(id, target.clone(), None, window, cx)
    });
    settle(&cx, |app| {
        let tab = docs.read(app).active_tab().expect("a tab");
        !tab.exporting && tab.session.state() == ely_files::SaveState::SaveFailed
    });
    assert!(!target.exists(), "the failure writes nothing");
    std::fs::remove_file(&work).expect("unblocks");
    std::fs::create_dir_all(&work).expect("the scratch returns");
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.write_as(id, target.clone(), None, window, cx)
    });
    written(&cx, &target, None);
    assert!(target.exists(), "a retry lands");
    cx.update(|app| {
        let state = docs.read(app).active_tab().expect("a tab").session.state();
        assert_eq!(
            state,
            ely_files::SaveState::Saved,
            "the retry clears the failure"
        );
    });
}

#[test]
fn an_edit_that_just_landed_still_asks_and_selection_does_not() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let Some(docs) = docs(&mut cx, dir.path()) else {
        eprintln!("SKIP: no LOK build");
        return;
    };
    let id = blank(&mut cx, &docs);
    commit(&mut cx, &docs, "JustLanded");
    let view = cx.update(|app| docs.read(app).active_tab().expect("a tab").view.clone());
    settle(&cx, |app| view.read(app).settled());
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.request_close(id, After::Tab, window, cx)
    });
    settle(&cx, |app| {
        matches!(docs.read(app).core.dialog, Some(Dialog::Unsaved { .. }))
    });
    assert!(
        cx.update(|app| docs.read(app).tab(id).is_some()),
        "a landed edit asks before closing"
    );

    let id = blank(&mut cx, &docs);
    let view = cx.update(|app| docs.read(app).active_tab().expect("a tab").view.clone());
    view.update(&mut cx, |view, _| {
        view.run("edit.select_all", &[]).expect("select all queues")
    });
    settle(&cx, |app| view.read(app).settled());
    with_window(&mut cx, &docs, |docs, window, cx| {
        docs.request_close(id, After::Tab, window, cx)
    });
    settle(&cx, |app| docs.read(app).tab(id).is_none());
}
