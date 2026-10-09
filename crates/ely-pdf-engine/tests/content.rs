//! Text and object edits against the real worker (PDF-03, PDF-04).

use std::path::Path;

use ely_pdf_engine::{
    Doc, Worker, Writing, Wrote,
    sample::{Extras, pdf_with},
};

fn worker() -> Worker {
    Worker::spawn_from(Path::new(env!("CARGO_BIN_EXE_ely-pdf-worker"))).expect("the worker")
}

/// A Letter page: two lines, a stamp, a picture.
fn opened(dir: &Path) -> (Worker, Doc) {
    let path = dir.join("doc.pdf");
    let extras = Extras { stamp: Some("STAMP"), picture: true, ..Extras::default() };
    std::fs::write(&path, pdf_with(&[(612.0, 792.0, &["Hello world", "Second line"])], &[], &extras)).expect("writes");
    let mut worker = worker();
    let doc = worker.open(&path).expect("opens").doc;
    (worker, doc)
}

fn page_text(worker: &mut Worker, doc: Doc) -> String {
    worker.text(doc, 0).expect("reads").iter().map(|c| c.c).collect()
}

/// Rectangles equal to a hundredth of a point.
fn near(got: [f32; 4], want: [f32; 4]) {
    assert!(got.iter().zip(want).all(|(a, b)| (a - b).abs() < 0.01), "{got:?} is not {want:?}");
}

/// A rewrite of block `block` as `text` in `font`.
fn rewrite(worker: &mut Worker, doc: Doc, block: usize, text: &str, font: &str) -> Writing {
    let read = worker.text_blocks(doc, 0).expect("blocks")[block].clone();
    Writing {
        block: Some(block),
        expected: read.text,
        text: text.into(),
        font: font.into(),
        size: read.size,
        rgb: read.rgb,
        bx: [read.rect[0], read.rect[1], read.rect[2] - read.rect[0]],
    }
}

#[test]
fn blocks_say_what_they_hold_and_whether_they_edit() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (mut worker, doc) = opened(dir.path());
    let blocks = worker.text_blocks(doc, 0).expect("blocks");
    let hello = blocks.iter().find(|b| b.text.starts_with("Hello")).expect("the page text");
    assert!(hello.editable && hello.size == 18.0, "{hello:?}");
    let stamp = blocks.iter().find(|b| b.text == "STAMP").expect("the stamp text");
    assert!(!stamp.editable, "text inside a form is not the page's own");
    let drawn = worker.objects(doc, 0).expect("objects");
    assert_eq!(drawn.iter().map(|d| d.form).collect::<Vec<_>>(), [true, false]);
    assert_eq!(drawn[1].rect, [72.0, 242.0, 172.0, 292.0], "the picture, in page space");
}

#[test]
fn edited_text_replaces_the_old_glyphs_in_one_step() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (mut worker, mut doc) = opened(dir.path());
    let before = worker.history(doc).expect("a mark");
    let index = worker.text_blocks(doc, 0).expect("blocks").iter().position(|b| b.text.starts_with("Hello")).expect("hello");
    let writing = rewrite(&mut worker, doc, index, "Hi world\nSecond line", "helvetica");
    assert!(matches!(worker.put_text(doc, 0, &writing, 0.0).expect("writes"), Wrote::Done(_)));
    doc = worker.now(doc).expect("the state");
    let now = page_text(&mut worker, doc);
    assert!(now.contains("Hi world") && !now.contains("Hello"), "{now}");
    worker.undo(doc, false).expect("undoes");
    doc = worker.now(doc).expect("the state");
    assert_eq!(worker.history(doc).expect("a mark"), before);
    assert!(page_text(&mut worker, doc).contains("Hello world"));
}

#[test]
fn a_font_without_the_glyph_changes_nothing() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (mut worker, mut doc) = opened(dir.path());
    let before = worker.history(doc).expect("a mark");
    let index = worker.text_blocks(doc, 0).expect("blocks").iter().position(|b| b.text.starts_with("Hello")).expect("hello");
    let writing = rewrite(&mut worker, doc, index, "你好", "helvetica");
    assert_eq!(worker.put_text(doc, 0, &writing, 0.0).expect("answers"), Wrote::Missing('你'));
    doc = worker.now(doc).expect("the state");
    assert_eq!(worker.history(doc).expect("a mark"), before);
    assert!(page_text(&mut worker, doc).contains("Hello world"), "the old text stays");
    let chinese = Writing { font: "cjk-sc".into(), ..writing };
    assert!(matches!(worker.put_text(doc, 0, &chinese, 0.0).expect("writes"), Wrote::Done(_)));
    doc = worker.now(doc).expect("the state");
    assert!(page_text(&mut worker, doc).contains("你好"));
}

#[test]
fn a_preview_draws_the_change_and_keeps_none_of_it() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (mut worker, mut doc) = opened(dir.path());
    let before = worker.history(doc).expect("a mark");
    let index = worker.text_blocks(doc, 0).expect("blocks").iter().position(|b| b.text.starts_with("Hello")).expect("hello");
    let mut writing = rewrite(&mut worker, doc, index, "A much longer line that will not fit the old box at all", "times");
    writing.bx[2] = 150.0;
    let Wrote::Preview(pixels, (w, h), grew) = worker.put_text(doc, 0, &writing, 2.0).expect("previews") else {
        panic!("a preview");
    };
    doc = worker.now(doc).expect("the state");
    assert!(grew, "more lines than before");
    assert_eq!(pixels.len(), (w * h * 4) as usize);
    assert!(pixels.chunks_exact(4).any(|p| p[0] < 128), "it draws text");
    assert_eq!(worker.history(doc).expect("a mark"), before);
    assert!(page_text(&mut worker, doc).contains("Hello world"));
}

#[test]
fn new_text_goes_where_asked() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (mut worker, mut doc) = opened(dir.path());
    let writing = Writing {
        block: None,
        expected: String::new(),
        text: "Added".into(),
        font: "courier".into(),
        size: 12.0,
        rgb: 0xff0000,
        bx: [100.0, 600.0, 300.0],
    };
    assert!(matches!(worker.put_text(doc, 0, &writing, 0.0).expect("writes"), Wrote::Done(_)));
    doc = worker.now(doc).expect("the state");
    let added = worker.text_blocks(doc, 0).expect("blocks").into_iter().find(|b| b.text == "Added").expect("added");
    assert!((added.rect[0] - 100.0).abs() < 1.0 && (added.rect[1] - 600.0).abs() < 3.0, "{added:?}");
    assert_eq!(added.rgb, 0xff0000);
}

#[test]
fn objects_move_scale_swap_and_go() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (mut worker, mut doc) = opened(dir.path());
    worker.place(doc, 0, 1, [1.0, 0.0, 0.0, 1.0, 50.0, 20.0]).expect("moves");
    doc = worker.now(doc).expect("the state");
    near(worker.objects(doc, 0).expect("objects")[1].rect, [122.0, 262.0, 222.0, 312.0]);
    // Twice the size about its top left.
    worker.place(doc, 0, 1, [2.0, 0.0, 0.0, 2.0, -122.0, -262.0]).expect("scales");
    doc = worker.now(doc).expect("the state");
    near(worker.objects(doc, 0).expect("objects")[1].rect, [122.0, 262.0, 322.0, 362.0]);
    let square = dir.path().join("square.pnm");
    std::fs::write(&square, b"P6\n2 2\n255\n\xff\xff\xff\x00\x00\x00\x00\x00\x00\xff\xff\xff").expect("writes");
    worker.replace_image(doc, 0, 1, &square).expect("replaces");
    doc = worker.now(doc).expect("the state");
    let swapped = worker.objects(doc, 0).expect("objects")[1].rect;
    near(swapped, [172.0, 262.0, 272.0, 362.0]);
    worker.delete_object(doc, 0, 0).expect("deletes the stamp");
    doc = worker.now(doc).expect("the state");
    assert!(!page_text(&mut worker, doc).contains("STAMP"));
    assert_eq!(worker.objects(doc, 0).expect("objects").len(), 1);
    worker.add_image(doc, 0, &square, [400.0, 100.0, 450.0, 150.0]).expect("adds");
    doc = worker.now(doc).expect("the state");
    near(worker.objects(doc, 0).expect("objects").last().expect("added").rect, [400.0, 100.0, 450.0, 150.0]);
    for _ in 0..5 {
        worker.undo(doc, false).expect("undoes");
        doc = worker.now(doc).expect("the state");
    }
    near(worker.objects(doc, 0).expect("objects")[1].rect, [72.0, 242.0, 172.0, 292.0]);
    assert!(page_text(&mut worker, doc).contains("STAMP"));
}

#[test]
fn the_documents_own_font_writes_and_edits_edit_again() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (mut worker, mut doc) = opened(dir.path());
    let hello = |worker: &mut Worker, doc: Doc, start: &str| {
        worker.text_blocks(doc, 0).expect("blocks").iter().position(|b| b.text.starts_with(start)).expect("the block")
    };
    let index = hello(&mut worker, doc, "Hello");
    let first = rewrite(&mut worker, doc, index, "Hi world\nSecond line", "original");
    assert!(matches!(worker.put_text(doc, 0, &first, 0.0).expect("writes"), Wrote::Done(_)));
    doc = worker.now(doc).expect("the state");
    let again = hello(&mut worker, doc, "Hi");
    assert!(worker.text_blocks(doc, 0).expect("blocks")[again].editable, "new text is the page's own");
    let second = rewrite(&mut worker, doc, again, "Yo world\nSecond line", "original");
    assert!(matches!(worker.put_text(doc, 0, &second, 0.0).expect("writes again"), Wrote::Done(_)));
    doc = worker.now(doc).expect("the state");
    let now = page_text(&mut worker, doc);
    assert!(now.contains("Yo world") && !now.contains("Hi world"), "{now}");
}

#[test]
fn a_font_added_in_a_preview_or_undone_is_added_again() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (mut worker, mut doc) = opened(dir.path());
    let index = worker.text_blocks(doc, 0).expect("blocks").iter().position(|b| b.text.starts_with("Hello")).expect("hello");
    let writing = rewrite(&mut worker, doc, index, "你好", "cjk-sc");
    assert!(matches!(worker.put_text(doc, 0, &writing, 2.0).expect("previews"), Wrote::Preview(..)));
    doc = worker.now(doc).expect("the state");
    assert!(matches!(worker.put_text(doc, 0, &writing, 0.0).expect("writes"), Wrote::Done(_)));
    doc = worker.now(doc).expect("the state");
    worker.undo(doc, false).expect("undoes");
    doc = worker.now(doc).expect("the state");
    assert!(matches!(worker.put_text(doc, 0, &writing, 0.0).expect("writes again"), Wrote::Done(_)));
    doc = worker.now(doc).expect("the state");
    let out = dir.path().join("out.pdf");
    worker.save(doc, &out).expect("saves");
    let saved = worker.open(&out).expect("reopens").doc;
    assert!(page_text(&mut worker, saved).contains("你好"), "the font is in the file");
}

#[test]
fn previews_and_refusals_keep_the_redo() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (mut worker, mut doc) = opened(dir.path());
    let index = worker.text_blocks(doc, 0).expect("blocks").iter().position(|b| b.text.starts_with("Hello")).expect("hello");
    let first = rewrite(&mut worker, doc, index, "Hi world\nSecond line", "helvetica");
    assert!(matches!(worker.put_text(doc, 0, &first, 0.0).expect("writes"), Wrote::Done(_)));
    doc = worker.now(doc).expect("the state");
    worker.undo(doc, false).expect("undoes");
    doc = worker.now(doc).expect("the state");
    let preview = rewrite(&mut worker, doc, index, "Yo world\nSecond line", "times");
    assert!(matches!(worker.put_text(doc, 0, &preview, 2.0).expect("previews"), Wrote::Preview(..)));
    doc = worker.now(doc).expect("the state");
    let refused = Writing { text: "你好".into(), font: "helvetica".into(), ..preview };
    assert!(matches!(worker.put_text(doc, 0, &refused, 0.0).expect("answers"), Wrote::Missing(_)));
    doc = worker.now(doc).expect("the state");
    worker.undo(doc, true).expect("the edit still redoes");
    doc = worker.now(doc).expect("the state");
    assert!(page_text(&mut worker, doc).contains("Hi world"));
}

#[test]
fn bigger_glyphs_count_as_a_new_layout() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (mut worker, mut doc) = opened(dir.path());
    let index = worker.text_blocks(doc, 0).expect("blocks").iter().position(|b| b.text.starts_with("Hello")).expect("hello");
    let same = rewrite(&mut worker, doc, index, "Hello world\nSecond line", "original");
    let Wrote::Preview(_, _, grew) = worker.put_text(doc, 0, &same, 2.0).expect("previews") else { panic!("a preview") };
    doc = worker.now(doc).expect("the state");
    assert!(!grew, "the same text in the same place");
    let big = Writing { text: "Hi".into(), size: 72.0, ..same };
    let Wrote::Preview(_, _, grew) = worker.put_text(doc, 0, &big, 2.0).expect("previews") else { panic!("a preview") };
    assert!(grew, "fewer lines, much bigger glyphs");
}

#[test]
fn runs_on_one_baseline_still_wrap_onto_new_lines() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let content = "BT /F1 18 Tf 1 0 0 1 72 720 Tm (Hello) Tj 1 0 0 1 160 720 Tm (world) Tj ET";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut bytes = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (n, obj) in objs.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{obj}\nendobj\n", n + 1).as_bytes());
    }
    let xref = bytes.len();
    bytes.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for offset in offsets {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
    let path = dir.path().join("runs.pdf");
    std::fs::write(&path, bytes).expect("writes");
    let mut worker = worker();
    let mut doc = worker.open(&path).expect("opens").doc;
    let block = worker.text_blocks(doc, 0).expect("blocks")[0].clone();
    let mut writing = rewrite(&mut worker, doc, 0, "One two three four five", "helvetica");
    writing.expected = block.text;
    writing.bx[2] = 50.0;
    assert!(matches!(worker.put_text(doc, 0, &writing, 0.0).expect("writes"), Wrote::Done(_)));
    doc = worker.now(doc).expect("the state");
    let mut tops: Vec<i32> = worker.text(doc, 0).expect("reads").iter().map(|c| c.rect[1].round() as i32).collect();
    tops.dedup();
    assert!(tops.len() >= 3, "each wrapped line has its own baseline: {tops:?}");
}

#[test]
fn a_subset_font_writes_with_its_own_glyphs_and_reads_back() {
    // LibreOffice's export: a subset font whose own tables are placeholders.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/office/t.pdf");
    let mut worker = worker();
    let mut doc = worker.open(&path).expect("opens").doc;
    let at = worker.text_blocks(doc, 0).expect("blocks").iter().position(|b| b.text.contains("BUNDLE")).expect("the block");
    let mut writing = rewrite(&mut worker, doc, at, "TEXT BUNDLE", "original");
    // Room for one line; the space stays a space.
    writing.bx[2] *= 2.0;
    assert!(matches!(worker.put_text(doc, 0, &writing, 0.0).expect("answers"), Wrote::Done(_)), "the page's glyphs suffice");
    doc = worker.now(doc).expect("the state");
    let now = page_text(&mut worker, doc);
    assert!(now.contains("TEXT BUNDLE"), "the new words read back: {now:?}");
}
