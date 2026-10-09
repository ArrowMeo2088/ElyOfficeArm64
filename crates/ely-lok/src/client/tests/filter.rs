//! An AutoFilter range keeps its rows (TABLE-08).

use super::sheet::open_book;
use super::*;
use crate::sheet;

/// A header, three numbers and their total right under them.
const BLOCK: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
 xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
 xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" office:version="1.3" office:mimetype="application/vnd.oasis.opendocument.spreadsheet">
 <office:body><office:spreadsheet><table:table table:name="S">
 <table:table-row><table:table-cell><text:p>Name</text:p></table:table-cell></table:table-row>
 <table:table-row><table:table-cell office:value-type="float" office:value="1"><text:p>1</text:p></table:table-cell></table:table-row>
 <table:table-row><table:table-cell office:value-type="float" office:value="2"><text:p>2</text:p></table:table-cell></table:table-row>
 <table:table-row><table:table-cell office:value-type="float" office:value="3"><text:p>3</text:p></table:table-cell></table:table-row>
 <table:table-row><table:table-cell office:value-type="float" office:value="6"><text:p>6</text:p></table:table-cell></table:table-row>
 </table:table></office:spreadsheet></office:body>
</office:document>"#;

/// Column A's filter entries.
fn entries(worker: &mut Worker, doc: &Doc) -> Vec<String> {
    let query = sheet::cell_query(sheet::FILTER, 0, 0);
    let json = worker.values(doc, &query).expect("the entries");
    sheet::filter_values(&json).expect("parses").entries.into_iter().map(|e| e.text).collect()
}

/// A book with an AutoFilter set on `marked`.
fn filtered(marked: &str) -> (tempfile::TempDir, Worker, Doc) {
    let (dir, mut worker, doc) = open_book(BLOCK);
    worker.run(&doc, "cell.goto", &[marked]).expect("marks the cells");
    worker.run(&doc, "data.autofilter", &[]).expect("filters");
    (dir, worker, doc)
}

/// Keeps 1: the saved range, and whether the total hides.
fn keep_one(worker: &mut Worker, doc: &Doc, dir: &std::path::Path) -> (String, bool) {
    worker.run(doc, "cell.goto", &["A1"]).expect("A1");
    let one = r#"{"column":0,"conditions":[{"op":"values","values":["1"]}]}"#;
    worker.run(doc, "data.filter", &[one]).expect("keeps 1");
    let out = dir.join("kept.fods");
    doc.save_as(worker, &crate::file_url(&out), "fods").expect("saves");
    let xml = std::fs::read_to_string(&out).expect("the saved book");
    let range = xml.split("table:target-range-address=\"").nth(1).expect("a filter range");
    let total = &xml[..xml.find("office:value=\"6\"").expect("the total")];
    let row = &total[total.rfind("<table:table-row").expect("its row")..];
    (range[..range.find('"').expect("closed")].to_string(), row.contains("table:visibility=\"filter\""))
}

#[test]
fn an_autofilter_on_a_marked_block_leaves_the_row_below_out() {
    if install().is_none() {
        eprintln!("SKIP: no pinned LOK build");
        return;
    }
    let (dir, mut worker, doc) = filtered("A1:A4");
    assert_eq!(entries(&mut worker, &doc), ["1", "2", "3"], "the total below stays out");
    assert_eq!(keep_one(&mut worker, &doc, dir.path()), ("S.A1:S.A4".into(), false), "applied from one cell");
}

#[test]
fn an_autofilter_on_one_cell_takes_the_data_area() {
    if install().is_none() {
        eprintln!("SKIP: no pinned LOK build");
        return;
    }
    let (_dir, mut worker, doc) = filtered("A1");
    assert_eq!(entries(&mut worker, &doc), ["1", "2", "3", "6"], "the whole column of data");
}

#[test]
fn off_and_undone_or_set_on_a_filtered_block_the_rows_stay() {
    if install().is_none() {
        eprintln!("SKIP: no pinned LOK build");
        return;
    }
    let (_dir, mut worker, doc) = filtered("A1:A4");
    worker.run(&doc, "cell.goto", &["A1"]).expect("A1");
    worker.run(&doc, "data.autofilter", &[]).expect("off");
    worker.run(&doc, "edit.undo", &[]).expect("back on");
    assert_eq!(entries(&mut worker, &doc), ["1", "2", "3"], "undo keeps the rows");
    let (_dir, mut worker, doc) = open_book(BLOCK);
    worker.run(&doc, "cell.goto", &["A1:A4"]).expect("marks the block");
    let one = r#"{"conditions":[{"field":0,"op":"equals","value":"1"}]}"#;
    worker.run(&doc, "data.filter", &[one]).expect("a standard filter");
    worker.run(&doc, "cell.goto", &["A1:A4"]).expect("marks it again");
    worker.run(&doc, "data.autofilter", &[]).expect("filters");
    assert_eq!(entries(&mut worker, &doc), ["1", "2", "3"], "a filtered block keeps its rows");
}

#[test]
fn a_saved_autofilter_range_keeps_its_rows_when_reopened() {
    if install().is_none() {
        eprintln!("SKIP: no pinned LOK build");
        return;
    }
    for format in ["xlsx", "ods"] {
        let (dir, mut worker, doc) = filtered("A1:A4");
        let path = dir.path().join(format!("book.{format}"));
        doc.save_as(&mut worker, &crate::file_url(&path), format).expect("saves");
        worker.close_doc(doc).expect("closes");
        let doc = worker.load(&crate::file_url(&path)).expect("reopens");
        assert_eq!(entries(&mut worker, &doc), ["1", "2", "3"], "{format}");
        assert_eq!(keep_one(&mut worker, &doc, dir.path()), ("S.A1:S.A4".into(), false), "{format}");
    }
}
