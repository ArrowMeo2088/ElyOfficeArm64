use std::path::Path;

use super::{Pictures, body, document, plain};

fn page(text: &str, beside: Option<&Path>, pictures: &Pictures) -> super::Page {
    body(text, beside, pictures).expect("a page")
}

#[test]
fn formulas_and_diagrams_draw_as_svg() {
    let text =
        "Inline $x^2$ here.\n\n$$\n\\sqrt{y}\n$$\n\n```mermaid\nflowchart LR\n  A --> B\n```\n";
    let page = page(text, None, &Pictures::Inline);
    assert!(!page.html.contains("language-math"), "{}", page.html);
    assert!(!page.html.contains("language-mermaid"), "{}", page.html);
    assert_eq!(page.html.matches("<svg").count(), 3, "{}", page.html);
    assert!(
        page.html.contains("<span class=\"math\"><svg")
            && page.html.contains("<div class=\"diagram\"><svg")
    );
    assert!(page.formulas.is_empty() && page.diagrams.is_empty());
}

#[test]
fn what_does_not_draw_stays_as_code_with_its_line() {
    let text = "# Title\n\n$$\n\\frac{\n$$\n\nText $\\sqrt{$ more.\n\n```mermaid\nflowchart LR\n  A -->\n```\n";
    let page = page(text, None, &Pictures::Inline);
    let lines: Vec<usize> = page.formulas.iter().map(|(line, _)| *line).collect();
    assert_eq!(lines, [3, 7]);
    assert!(page.formulas.iter().all(|(_, why)| !why.is_empty()));
    assert_eq!(page.diagrams, [9]);
    assert!(
        page.html.contains("language-math math-display\">\\frac{"),
        "{}",
        page.html
    );
    assert!(
        page.html
            .contains("language-math math-inline\">\\sqrt{</code>"),
        "{}",
        page.html
    );
    assert!(
        page.html.contains("language-mermaid\">flowchart LR"),
        "{}",
        page.html
    );
}

#[test]
fn an_undrawn_formula_after_a_drawn_one_keeps_its_code() {
    let page = page("$$\nx\n$$\n$$\n\\frac{\n$$\n", None, &Pictures::Inline);
    assert_eq!(page.formulas.len(), 1);
    assert!(page.html.contains("\\frac{"), "{}", page.html);
}

#[test]
fn front_matter_and_raw_html_do_not_go_live() {
    let page = page(
        "---\ntitle: Secret\n---\n\n<script>alert(1)</script>\n\nText.\n",
        None,
        &Pictures::Inline,
    );
    assert!(!page.html.contains("Secret"), "{}", page.html);
    assert!(!page.html.contains("<script>"), "{}", page.html);
    assert!(page.html.contains("Text."));
}

#[test]
fn inline_pictures_are_data_and_web_ones_stay() {
    let dir = tempfile::tempdir().expect("a dir");
    std::fs::write(dir.path().join("dot.png"), b"png bytes").expect("writes");
    let text = "![a](dot.png) ![b](gone.png) ![c](https://example.com/c.png?x=1&y=2)\n";
    let page = page(text, Some(dir.path()), &Pictures::Inline);
    assert!(
        page.html
            .contains("src=\"data:image/png;base64,cG5nIGJ5dGVz\""),
        "{}",
        page.html
    );
    assert!(
        page.html
            .contains("src=\"https://example.com/c.png?x=1&amp;y=2\""),
        "{}",
        page.html
    );
    assert_eq!(page.missing, ["gone.png"]);
    assert!(page.copies.is_empty());
}

#[test]
fn folder_pictures_copy_once_under_names_of_their_own() {
    let dir = tempfile::tempdir().expect("a dir");
    for folder in ["one", "two"] {
        std::fs::create_dir(dir.path().join(folder)).expect("makes");
        std::fs::write(dir.path().join(folder).join("my pic.png"), folder).expect("writes");
    }
    let text = "![a](one/my%20pic.png) ![b](two/my%20pic.png) ![c](<one/my pic.png>)\n";
    let page = page(
        text,
        Some(dir.path()),
        &Pictures::Folder("doc_files".into()),
    );
    let names: Vec<&str> = page.copies.iter().map(|(_, name)| name.as_str()).collect();
    assert_eq!(names, ["my pic.png", "my pic-2.png"]);
    assert_eq!(
        page.html.matches("src=\"doc_files/my%20pic.png\"").count(),
        2,
        "{}",
        page.html
    );
    assert!(
        page.html.contains("src=\"doc_files/my%20pic-2.png\""),
        "{}",
        page.html
    );
}

#[test]
fn a_pdf_leaves_out_what_mupdf_cannot_read() {
    let dir = tempfile::tempdir().expect("a dir");
    std::fs::write(dir.path().join("a.webp"), b"w").expect("writes");
    std::fs::write(dir.path().join("b.png"), b"p").expect("writes");
    let text = "![a](a.webp) ![b](b.png) ![c](https://example.com/c.png)\n";
    let page = page(text, Some(dir.path()), &Pictures::Pdf("files".into()));
    assert_eq!(page.missing, ["a.webp", "https://example.com/c.png"]);
    assert_eq!(page.copies.len(), 1);
}

#[test]
fn a_blocked_address_is_missing_by_its_text() {
    let page = page("![the chart](file:///tmp/c.png)\n", None, &Pictures::Inline);
    assert_eq!(page.missing, ["the chart"]);
}

#[test]
fn an_untitled_document_has_no_folder_for_pictures() {
    let page = page("![a](a.png)\n", None, &Pictures::Inline);
    assert_eq!(page.missing, ["a.png"]);
}

#[test]
fn a_document_wraps_the_body_and_escapes_its_title() {
    let html = document("a<b & c", "<p>x</p>\n");
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<title>a&lt;b &amp; c</title>") && html.contains("<p>x</p>"));
}

#[test]
fn plain_text_keeps_words_code_and_tex() {
    let text = "---\ntitle: x\n---\n\n# Head *one*\n\n- a\n- b `c`\n\n| h | i |\n|---|---|\n| 1 | 2 |\n\n$$\n\\sqrt{x}\n$$\n\n> quoted $y$\n";
    assert_eq!(
        plain(text).expect("text"),
        "Head one\n\na\nb c\n\nh\ti\n1\t2\n\n\\sqrt{x}\n\nquoted y\n"
    );
}

mod files {
    use std::path::Path;

    use super::super::{Format, Target, write};

    fn target<'a>(path: &'a Path, work: &'a Path) -> Target<'a> {
        Target {
            path,
            session: None,
            revision: 1,
            work,
        }
    }

    fn picture(dir: &Path) {
        std::fs::write(dir.join("dot.png"), b"png").expect("writes");
    }

    #[test]
    fn a_page_carries_its_pictures_inside() {
        let dir = tempfile::tempdir().expect("a dir");
        picture(dir.path());
        let out = dir.path().join("out.html");
        write(
            Format::Page,
            "# Hi\n\n![d](dot.png)\n",
            Some(dir.path()),
            &target(&out, dir.path()),
        )
        .expect("writes");
        let html = std::fs::read_to_string(&out).expect("reads");
        assert!(html.starts_with("<!doctype html>") && html.contains("<title>out</title>"));
        assert!(html.contains("data:image/png;base64,"), "{html}");
    }

    #[test]
    fn a_linked_page_copies_into_a_new_folder_only() {
        let dir = tempfile::tempdir().expect("a dir");
        picture(dir.path());
        let out = dir.path().join("out.html");
        write(
            Format::Linked,
            "![d](dot.png)\n",
            Some(dir.path()),
            &target(&out, dir.path()),
        )
        .expect("writes");
        assert_eq!(
            std::fs::read(dir.path().join("out_files/dot.png")).expect("copied"),
            b"png"
        );
        assert!(
            std::fs::read_to_string(&out)
                .expect("reads")
                .contains("src=\"out_files/dot.png\"")
        );
        std::fs::remove_file(&out).expect("removes");
        let error = write(
            Format::Linked,
            "![d](dot.png)\n",
            Some(dir.path()),
            &target(&out, dir.path()),
        )
        .expect_err("the folder is there");
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert!(error.to_string().contains("out_files"), "{error}");
        assert!(!out.exists(), "nothing written");
    }

    #[test]
    fn a_linked_page_without_pictures_makes_no_folder() {
        let dir = tempfile::tempdir().expect("a dir");
        let out = dir.path().join("out.html");
        write(
            Format::Linked,
            "Text.\n",
            Some(dir.path()),
            &target(&out, dir.path()),
        )
        .expect("writes");
        assert!(!dir.path().join("out_files").exists());
    }

    #[test]
    fn text_is_plain_and_the_open_file_is_never_written() {
        let dir = tempfile::tempdir().expect("a dir");
        let out = dir.path().join("doc.md");
        std::fs::write(&out, "# Kept\n").expect("writes");
        let mut aimed = target(&out, dir.path());
        aimed.session = Some(&out);
        write(Format::Text, "# Other\n", None, &aimed).expect_err("refused");
        assert_eq!(std::fs::read_to_string(&out).expect("reads"), "# Kept\n");
        let txt = dir.path().join("doc.txt");
        write(
            Format::Text,
            "# Title *here*\n",
            None,
            &target(&txt, dir.path()),
        )
        .expect("writes");
        assert_eq!(
            std::fs::read_to_string(&txt).expect("reads"),
            "Title here\n"
        );
    }

    #[test]
    fn a_pdf_sets_the_page_and_leaves_no_scratch() {
        let dir = tempfile::tempdir().expect("a dir");
        let work = tempfile::tempdir().expect("work");
        let out = dir.path().join("out.pdf");
        let text = "# Heading 标题\n\nInline $x^2$.\n\n```mermaid\nflowchart LR\n  A --> B\n```\n";
        let page = write(
            Format::Pdf,
            text,
            Some(dir.path()),
            &target(&out, work.path()),
        )
        .expect("writes");
        assert!(page.missing.is_empty() && page.formulas.is_empty() && page.diagrams.is_empty());
        assert!(!work.path().join("pdf").exists(), "the scratch is gone");
        let mut worker = ely_pdf_engine::Worker::spawn().expect("the worker");
        let doc = worker.open(&out).expect("opens").doc;
        let words: String = worker
            .text(doc, 0)
            .expect("text")
            .iter()
            .map(|ch| ch.c)
            .collect();
        assert!(
            words.contains("Heading") && words.contains("标题"),
            "{words:?}"
        );
    }
}
