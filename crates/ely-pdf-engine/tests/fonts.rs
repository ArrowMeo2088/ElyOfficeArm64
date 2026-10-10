//! Fonts the editor meets in the wild (PDF-03).

use std::path::Path;

use ely_pdf_engine::{Doc, Worker, Writing, Wrote};

fn worker() -> Worker {
    Worker::spawn_from(Path::new(env!("CARGO_BIN_EXE_ely-pdf-worker"))).expect("the worker")
}

fn page_text(worker: &mut Worker, doc: Doc) -> String {
    worker.text(doc, 0).expect("reads").iter().map(|c| c.c).collect()
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

/// A page whose text uses a CMap-encoded CID font.
fn cmap_font_pdf() -> Vec<u8> {
    let content = "BT /F1 24 Tf 72 700 Td <00480069> Tj ET";
    one_font_page(content, &[
        "<< /Type /Font /Subtype /Type0 /BaseFont /STSong-Light /Encoding /UniGB-UCS2-H /DescendantFonts [6 0 R] >>".to_string(),
        "<< /Type /Font /Subtype /CIDFontType0 /BaseFont /STSong-Light /CIDSystemInfo << /Registry (Adobe) /Ordering (GB1) /Supplement 2 >> /FontDescriptor 7 0 R >>".to_string(),
        "<< /Type /FontDescriptor /FontName /STSong-Light /Flags 4 /FontBBox [0 -200 1000 900] /ItalicAngle 0 /Ascent 880 /Descent -120 /CapHeight 700 /StemV 80 >>".to_string(),
    ])
}

/// A page drawing `content`; font objects start at 5.
fn one_font_page(content: &str, font: &[String]) -> Vec<u8> {
    page_with("<< /F1 5 0 R >>", content, font)
}

/// A page with font resources `fonts`; objects from 5.
fn page_with(fonts: &str, content: &str, font: &[String]) -> Vec<u8> {
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font {fonts} >> /Contents 4 0 R >>"),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
    ];
    objects.extend_from_slice(font);
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend(format!("{} 0 obj\n{body}\nendobj\n", i + 1).bytes());
    }
    let xref = pdf.len();
    pdf.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
    for offset in offsets {
        pdf.extend(format!("{offset:010} 00000 n \n").bytes());
    }
    pdf.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).bytes());
    pdf
}

#[test]
fn a_font_whose_codes_ely_cannot_write_says_so() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let path = dir.path().join("cmap.pdf");
    std::fs::write(&path, cmap_font_pdf()).expect("writes");
    let mut worker = worker();
    let doc = worker.open(&path).expect("opens").doc;
    assert_eq!(page_text(&mut worker, doc), "Hi");
    let writing = rewrite(&mut worker, doc, 0, "Yo", "original");
    assert_eq!(worker.put_text(doc, 0, &writing, 2.0).expect("answers"), Wrote::Unwritable);
    assert_eq!(page_text(&mut worker, doc), "Hi", "nothing changed");
    let helvetica = rewrite(&mut worker, doc, 0, "Yo", "helvetica");
    assert!(matches!(worker.put_text(doc, 0, &helvetica, 0.0).expect("writes"), Wrote::Done(_)));
}

#[test]
fn a_font_written_into_the_page_says_it_cannot_write() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let path = dir.path().join("direct.pdf");
    // The font dictionary sits in the page itself.
    let fonts = "<< /F1 << /Type /Font /Subtype /Type0 /BaseFont /STSong-Light /Encoding /Identity-H /DescendantFonts [5 0 R] >> >>";
    let pdf = page_with(fonts, "BT /F1 24 Tf 72 700 Td <00480069> Tj ET", &[
        "<< /Type /Font /Subtype /CIDFontType0 /BaseFont /STSong-Light /CIDSystemInfo << /Registry (Adobe) /Ordering (GB1) /Supplement 2 >> /FontDescriptor 6 0 R >>".to_string(),
        "<< /Type /FontDescriptor /FontName /STSong-Light /Flags 4 /FontBBox [0 -200 1000 900] /ItalicAngle 0 /Ascent 880 /Descent -120 /CapHeight 700 /StemV 80 >>".to_string(),
    ]);
    std::fs::write(&path, pdf).expect("writes");
    let mut worker = worker();
    let doc = worker.open(&path).expect("opens").doc;
    let writing = rewrite(&mut worker, doc, 0, "Yo", "original");
    assert_eq!(worker.put_text(doc, 0, &writing, 2.0).expect("answers"), Wrote::Unwritable);
    let helvetica = rewrite(&mut worker, doc, 0, "Yo", "helvetica");
    assert!(matches!(worker.put_text(doc, 0, &helvetica, 0.0).expect("writes"), Wrote::Done(_)));
}
