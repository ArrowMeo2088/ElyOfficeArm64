//! An AutoFilter keeps to the marked block (TABLE-08).

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

/// The filter entries after an AutoFilter on `marked`.
fn entries_after(marked: &str) -> Vec<String> {
    let (_dir, mut worker, doc) = open_book(BLOCK);
    worker.run(&doc, "cell.goto", &[marked]).expect("marks the cells");
    worker.run(&doc, "data.autofilter", &[]).expect("filters");
    let query = sheet::cell_query(sheet::FILTER, 0, 0);
    let json = worker.values(&doc, &query).expect("the entries");
    sheet::filter_values(&json).expect("parses").entries.into_iter().map(|e| e.text).collect()
}

#[test]
fn an_autofilter_on_a_marked_block_leaves_the_row_below_out() {
    if install().is_none() {
        eprintln!("SKIP: no pinned LOK build");
        return;
    }
    assert_eq!(entries_after("A1:A4"), ["1", "2", "3"], "the total below stays out");
}

#[test]
fn an_autofilter_on_one_cell_takes_the_data_area() {
    if install().is_none() {
        eprintln!("SKIP: no pinned LOK build");
        return;
    }
    assert_eq!(entries_after("A1"), ["1", "2", "3", "6"], "the whole column of data");
}
