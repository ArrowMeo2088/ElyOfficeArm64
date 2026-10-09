//! Sheet numbers hold only in the state they were read.

use super::*;

#[test]
fn a_rename_asked_before_a_delete_renames_nothing() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let Some((window, book)) = book(&mut cx, dir.path()) else {
        eprintln!("SKIP: no LOK build");
        return;
    };
    blank(&mut cx, &book);
    for (name, at) in [("B", "2"), ("C", "3")] {
        fresh(&cx, &book);
        update(&mut cx, &book, |book, cx| book.run_active("sheet.insert", &[name, at], cx));
    }
    settle(&cx, |app| sheet_names(app, &book).len() == 3);
    fresh(&cx, &book);
    update(&mut cx, &book, |book, cx| book.show_sheet(1, cx));
    cx.update_window(window.into(), |_, window, app| book.update(app, |book, cx| book.ask_rename(window, cx)))
        .expect("the window lives");
    // The first sheet goes while the dialog is open.
    update(&mut cx, &book, |book, cx| book.run_active("sheet.delete", &["1"], cx));
    settle(&cx, |app| sheet_names(app, &book) == ["B", "C"]);
    fresh(&cx, &book);
    update(&mut cx, &book, |book, cx| book.rename_asked("Renamed B", cx));
    fresh(&cx, &book);
    assert_eq!(cx.update(|app| sheet_names(app, &book)), ["B", "C"], "C keeps its name");
}

#[test]
fn a_failed_command_still_moves_the_state_on() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut cx = TestAppContext::single();
    let Some((_window, book)) = book(&mut cx, dir.path()) else {
        eprintln!("SKIP: no LOK build");
        return;
    };
    blank(&mut cx, &book);
    // No merged cell to split: the kernel refuses.
    update(&mut cx, &book, |book, cx| book.run_active("cells.split", &[], cx));
    fresh(&cx, &book);
    update(&mut cx, &book, |book, cx| book.run_active("sheet.rename", &["Plan", "1"], cx));
    settle(&cx, |app| sheet_names(app, &book) == ["Plan"]);
}
