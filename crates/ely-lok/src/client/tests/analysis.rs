use super::rules::row;
use super::sheet::open_book;
use super::*;
use crate::sheet::{self, Layout, Look};

/// Region, Year, Sales; four rows.
fn data() -> String {
    let rows: String = [
        ["Region", "Year", "Sales"],
        ["East", "2025", "3"],
        ["West", "2025", "1"],
        ["East", "2026", "4"],
        ["West", "2026", "2"],
    ]
    .iter()
    .map(|cells| row(cells))
    .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
 xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
 xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" office:version="1.3" office:mimetype="application/vnd.oasis.opendocument.spreadsheet">
 <office:body><office:spreadsheet><table:table table:name="S">{rows}</table:table></office:spreadsheet></office:body>
</office:document>"#
    )
}

pub(super) fn cells(worker: &mut Worker, doc: &Doc, range: &str) -> String {
    worker.run(doc, "cell.goto", &[range]).expect("the range");
    doc.selection_text(worker).expect("the text")
}

/// `range` as the kernel writes it.
fn written(worker: &mut Worker, doc: &Doc, range: &str) -> String {
    worker.run(doc, "cell.goto", &[range]).expect("the range");
    let read = worker.values(doc, sheet::SELECTED).expect("the selection");
    sheet::selected_range(&read)
        .expect("parses")
        .expect("one range")
        .range
}

fn pivot_here(worker: &mut Worker, doc: &Doc) -> Option<sheet::Pivot> {
    let read = worker.values(doc, sheet::PIVOT).expect("the pivot read");
    sheet::pivot(&read).expect("parses")
}

fn chart_here(worker: &mut Worker, doc: &Doc) -> Option<sheet::Chart> {
    let read = worker.values(doc, sheet::CHART).expect("the chart read");
    sheet::chart(&read).expect("parses")
}

/// Undo steps in the kernel's history.
pub(super) fn steps(worker: &mut Worker, doc: &Doc) -> usize {
    let json = worker.values(doc, ".uno:Undo").expect("the history");
    let value: serde_json::Value = serde_json::from_str(&json).expect("parses");
    value["actions"].as_array().map_or(0, Vec::len)
}

pub(super) fn skip() -> bool {
    let missing = install().is_none();
    if missing {
        eprintln!("SKIP: no pinned LOK build");
    }
    missing
}

#[test]
fn pivot_tables_make_change_and_refresh() {
    if skip() {
        return;
    }
    let (_dir, mut worker, doc) = open_book(&data());
    let (source, dest, other) = (
        written(&mut worker, &doc, "A1:C5"),
        written(&mut worker, &doc, "E1"),
        written(&mut worker, &doc, "K1"),
    );
    worker
        .run(&doc, "cell.goto", &["A1:C7"])
        .expect("past the data");
    let read = worker.values(&doc, sheet::SELECTED).expect("the selection");
    let wide = sheet::selected_range(&read).expect("parses").expect("one");
    assert_eq!(wide.data, source, "the data inside, without empty rows");
    worker.run(&doc, "cell.goto", &["E1"]).expect("E1");
    assert_eq!(pivot_here(&mut worker, &doc), None, "no pivot yet");
    let layout = Layout {
        rows: vec!["Region".into()],
        data: vec![("Sales".into(), "sum".into())],
        ..Layout::default()
    };
    let spec = layout.to_json(&source, &dest);
    worker.run(&doc, "pivot.make", &[&spec]).expect("makes");
    let made = cells(&mut worker, &doc, "E1:F4");
    assert!(
        made.contains("East\t7") && made.contains("West\t3"),
        "{made:?}"
    );
    worker.run(&doc, "cell.goto", &["E2"]).expect("on it");
    let here = pivot_here(&mut worker, &doc).expect("a pivot");
    assert_eq!(here.source, source);
    assert_eq!(here.fields, ["Region", "Year", "Sales"]);
    // The kernel places the data layout field in the columns.
    let placed = Layout {
        columns: vec![String::new()],
        ..layout.clone()
    };
    assert_eq!(here.layout, placed);

    let changed = Layout {
        columns: vec!["Year".into(), String::new()],
        data: vec![("Sales".into(), "max".into())],
        ..placed
    };
    worker
        .run(&doc, "pivot.make", &[&changed.to_json("", "")])
        .expect("changes");
    worker.run(&doc, "cell.goto", &["E2"]).expect("on it");
    let here = pivot_here(&mut worker, &doc).expect("still a pivot");
    assert_eq!(here.layout, changed, "fields and function changed in place");

    worker.run(&doc, "cell.goto", &["C2"]).expect("C2");
    worker.run(&doc, "cell.enter", &["9"]).expect("enters");
    worker.run(&doc, "cell.goto", &["E3"]).expect("on it");
    worker.run(&doc, "pivot.refresh", &[]).expect("refreshes");
    let fresh = cells(&mut worker, &doc, "E1:H5");
    assert!(
        fresh.contains("East\t9\t4"),
        "the new value shows: {fresh:?}"
    );

    let bad = Layout {
        data: vec![("Nope".into(), "sum".into())],
        ..Layout::default()
    };
    worker.run(&doc, "cell.goto", &["K1"]).expect("K1");
    assert!(
        worker
            .run(&doc, "pivot.make", &[&bad.to_json(&source, &other)])
            .is_err(),
        "an unknown field fails"
    );
}

#[test]
fn charts_insert_and_change_in_one_step() {
    if skip() {
        return;
    }
    let (_dir, mut worker, doc) = open_book(&data());
    let (all, part) = (
        written(&mut worker, &doc, "A1:C5"),
        written(&mut worker, &doc, "A1:C3"),
    );
    worker.run(&doc, "cell.goto", &["A1"]).expect("A1");
    assert_eq!(chart_here(&mut worker, &doc), None, "no chart yet");
    let look = Look {
        kind: "column".into(),
        title: "Sales".into(),
        legend: true,
        x_title: "Region".into(),
        y_title: "Units".into(),
    };
    worker
        .run(&doc, "chart.insert", &[&all, "false", &look.to_json()])
        .expect("inserts");
    let chart = chart_here(&mut worker, &doc).expect("the new chart is selected");
    assert_eq!((&chart.look, &chart.range), (&look, &all));

    let pie = Look {
        kind: "pie".into(),
        title: String::new(),
        legend: false,
        ..Look::default()
    };
    worker
        .run(
            &doc,
            "chart.change",
            &[&chart.name, &part, "true", "true", &pie.to_json()],
        )
        .expect("changes");
    let changed = chart_here(&mut worker, &doc).expect("still selected");
    assert_eq!(changed.look, pie);
    assert_eq!(changed.range, part);
    worker.run(&doc, "edit.undo", &[]).expect("one undo");
    let back = chart_here(&mut worker, &doc).expect("still selected");
    assert_eq!((back.look, back.range), (look.clone(), chart.range.clone()));

    let before = steps(&mut worker, &doc);
    for bad in ["bad range", "NoSheet!A1:B3"] {
        let change = [chart.name.as_str(), bad, "true", "true", &pie.to_json()];
        assert!(
            worker.run(&doc, "chart.change", &change).is_err(),
            "{bad} changes nothing"
        );
        worker.run(&doc, "cell.goto", &["A1"]).expect("A1");
        let insert = [bad, "false", &look.to_json()];
        assert!(
            worker.run(&doc, "chart.insert", &insert).is_err(),
            "{bad} inserts nothing"
        );
    }
    assert_eq!(steps(&mut worker, &doc), before, "nothing changed");

    let wrong = Look {
        kind: "donut".into(),
        ..pie
    };
    assert!(
        worker
            .run(
                &doc,
                "chart.change",
                &[&chart.name, &part, "true", "true", &wrong.to_json()],
            )
            .is_err(),
        "an unknown kind fails"
    );
}

#[test]
fn a_read_layout_applies_back_unchanged() {
    if skip() {
        return;
    }
    let (_dir, mut worker, doc) = open_book(&data());
    let (source, dest) = (
        written(&mut worker, &doc, "A1:C5"),
        written(&mut worker, &doc, "E1"),
    );
    let across = Layout {
        rows: vec!["Region".into()],
        columns: vec![String::new()],
        data: vec![
            ("Sales".into(), "sum".into()),
            ("Year".into(), "max".into()),
        ],
        ..Layout::default()
    };
    worker
        .run(&doc, "pivot.make", &[&across.to_json(&source, &dest)])
        .expect("makes");
    let before = cells(&mut worker, &doc, "E1:G4");
    worker.run(&doc, "cell.goto", &["E2"]).expect("on it");
    let read = pivot_here(&mut worker, &doc).expect("a pivot").layout;
    assert_eq!(read, across, "the data field reads in place");
    worker
        .run(&doc, "pivot.make", &[&read.to_json("", "")])
        .expect("applies");
    assert_eq!(cells(&mut worker, &doc, "E1:G4"), before, "nothing moved");
}

/// The data with a pivot table that sums Region's subtotals.
fn with_subtotals() -> String {
    let table = r#"<table:data-pilot-tables><table:data-pilot-table table:name="P" table:target-range-address="S.E1:G14"><table:source-cell-range table:cell-range-address="S.A1:C5"/><table:data-pilot-field table:source-field-name="Region" table:orientation="row"><table:data-pilot-level table:show-empty="false"><table:data-pilot-subtotals><table:data-pilot-subtotal table:function="sum"/></table:data-pilot-subtotals></table:data-pilot-level></table:data-pilot-field><table:data-pilot-field table:source-field-name="Year" table:orientation="row"><table:data-pilot-level table:show-empty="false"/></table:data-pilot-field><table:data-pilot-field table:source-field-name="Sales" table:orientation="data" table:function="sum"/><table:data-pilot-field table:source-field-name="" table:is-data-layout-field="true" table:orientation="column"/></table:data-pilot-table></table:data-pilot-tables>"#;
    data().replace(
        "</office:spreadsheet>",
        &format!("{table}</office:spreadsheet>"),
    )
}

#[test]
fn a_read_layout_keeps_subtotals() {
    if skip() {
        return;
    }
    let (_dir, mut worker, doc) = open_book(&with_subtotals());
    worker.run(&doc, "cell.goto", &["E3"]).expect("on it");
    worker.run(&doc, "pivot.refresh", &[]).expect("refreshes");
    let before = cells(&mut worker, &doc, "E1:G14");
    assert!(
        before.contains("East Sum - Sales"),
        "the subtotal shows: {before:?}"
    );
    worker.run(&doc, "cell.goto", &["E3"]).expect("on it");
    let read = pivot_here(&mut worker, &doc).expect("a pivot").layout;
    worker
        .run(&doc, "pivot.make", &[&read.to_json("", "")])
        .expect("applies");
    assert_eq!(cells(&mut worker, &doc, "E1:G14"), before, "nothing moved");
}

/// The book saved as flat XML, read back as text.
fn saved(worker: &mut Worker, doc: &Doc, path: &std::path::Path) -> String {
    doc.save_as(worker, &crate::file_url(path), "fods")
        .expect("saves");
    std::fs::read_to_string(path).expect("reads")
}

#[test]
fn undoing_a_new_kind_brings_the_whole_chart_back() {
    if skip() {
        return;
    }
    let (dir, mut worker, doc) = open_book(&data());
    let all = written(&mut worker, &doc, "A1:C5");
    let look = Look {
        kind: "column".into(),
        legend: true,
        ..Look::default()
    };
    worker
        .run(&doc, "chart.insert", &[&all, "false", &look.to_json()])
        .expect("inserts");
    let name = chart_here(&mut worker, &doc).expect("selected").name;
    let flat = dir.path().join("stacked.fods");
    let text = saved(&mut worker, &doc, &flat);
    // The plot area's style gains stacking.
    let plot = r#"chart:include-hidden-cells="false""#;
    assert!(text.contains(plot), "the plot area style");
    let stacked = format!(r#"chart:stacked="true" {plot}"#);
    std::fs::write(&flat, text.replace(plot, &stacked)).expect("stacks it");
    worker.close_doc(doc).expect("closes");
    let doc = worker.load(&crate::file_url(&flat)).expect("reloads");
    let before = saved(&mut worker, &doc, &dir.path().join("before.fods"));
    assert!(
        before.contains(r#"chart:stacked="true""#),
        "the kernel reads it stacked"
    );
    let pie = Look {
        kind: "pie".into(),
        ..look
    };
    let change = [name.as_str(), &all, "true", "true", &pie.to_json()];
    worker.run(&doc, "chart.change", &change).expect("changes");
    worker.run(&doc, "edit.undo", &[]).expect("one undo");
    let back = saved(&mut worker, &doc, &dir.path().join("back.fods"));
    assert!(
        back.contains(r#"chart:stacked="true""#),
        "the stacking came back"
    );
}
