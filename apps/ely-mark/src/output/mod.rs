//! Markdown as an HTML page or plain text (MARK-11).

mod files;
mod pictures;
mod plain;
#[cfg(test)]
mod tests;

use std::{io, path::Path};

use ely_gpui_component::{
    documents::{diagram_svg, markdown_options},
    typography::formula_svg,
};
use markdown::{CompileOptions, Options, mdast::Node};

pub use files::{Format, Target, write};
pub use pictures::Pictures;
pub use plain::plain;

/// A page and what it could not carry.
#[derive(Debug, Default)]
pub struct Page {
    pub html: String,
    /// Pictures to copy beside the page: from, then name.
    pub copies: Vec<(std::path::PathBuf, String)>,
    /// Pictures left out, as the document wrote them.
    pub missing: Vec<String>,
    /// Lines of diagrams kept as code.
    pub diagrams: Vec<usize>,
    /// Lines of formulas kept as code, and why.
    pub formulas: Vec<(usize, String)>,
}

const DISPLAY: &str = "<pre><code class=\"language-math math-display\">";
const INLINE: &str = "<code class=\"language-math math-inline\">";
const MERMAID: &str = "<pre><code class=\"language-mermaid\">";
const IMAGE: &str = "<img src=\"";

/// The page's look: light, as printed.
const STYLE: &str = "body{font-family:sans-serif;font-size:11pt;line-height:1.5;color:#1f2328;max-width:48rem;margin:2rem auto;padding:0 1rem}\
pre{background:#f6f8fa;padding:.75em;white-space:pre-wrap}\
code{font-family:monospace}\
table{border-collapse:collapse}th,td{border:1px solid #d0d7de;padding:.3em .6em}\
blockquote{margin-left:0;padding-left:1em;border-left:3px solid #d0d7de;color:#59636e}\
img,svg{max-width:100%}div.math{text-align:center}span.math svg{vertical-align:middle}";

/// `text`'s body as HTML; pictures go as `pictures` says.
pub fn body(text: &str, beside: Option<&Path>, pictures: &Pictures) -> io::Result<Page> {
    let options = Options {
        parse: markdown_options(),
        compile: CompileOptions::gfm(),
    };
    let html = markdown::to_html_with_options(text, &options)
        .map_err(|error| io::Error::other(error.to_string()))?;
    let tree = markdown::to_mdast(text, &options.parse)
        .map_err(|error| io::Error::other(error.to_string()))?;
    let mut page = Page::default();
    let mut placed = pictures::Placed::new(beside, pictures);
    let mut rest = html.as_str();
    while let Some((at, mark)) = [DISPLAY, INLINE, MERMAID, IMAGE]
        .iter()
        .filter_map(|mark| rest.find(mark).map(|at| (at, *mark)))
        .min()
    {
        page.html.push_str(&rest[..at]);
        rest = &rest[at + mark.len()..];
        let close = match mark {
            DISPLAY | MERMAID => "</code></pre>",
            INLINE => "</code>",
            _ => "\"",
        };
        let end = rest
            .find(close)
            .ok_or_else(|| io::Error::other("markdown-rs left an element open"))?;
        let inside = &rest[..end];
        rest = &rest[end + close.len()..];
        let source = unescape(inside);
        if mark == IMAGE {
            page.html.push_str(IMAGE);
            page.html
                .push_str(&placed.place(&source, rest, &mut page.missing));
            page.html.push('"');
            continue;
        }
        let drawn = match mark {
            MERMAID => match diagram_svg(&source) {
                Ok(svg) => Some(format!("<div class=\"diagram\">{}</div>", bare(&svg))),
                Err(_) => {
                    page.diagrams.push(line(&tree, &source, true));
                    None
                }
            },
            _ => match formula_svg(&source, mark == DISPLAY) {
                Ok(svg) if mark == DISPLAY => {
                    Some(format!("<div class=\"math\">{}</div>", bare(&svg)))
                }
                Ok(svg) => Some(format!("<span class=\"math\">{}</span>", bare(&svg))),
                Err(error) => {
                    page.formulas
                        .push((line(&tree, &source, false), error.to_string()));
                    None
                }
            },
        };
        // What did not draw stays as written.
        page.html += &drawn.unwrap_or_else(|| format!("{mark}{inside}{close}"));
    }
    page.html.push_str(rest);
    page.copies = placed.copies;
    log::info!(
        "mark: a page of {} bytes, {} copies, {} missing, {} diagrams and {} formulas as code",
        page.html.len(),
        page.copies.len(),
        page.missing.len(),
        page.diagrams.len(),
        page.formulas.len()
    );
    Ok(page)
}

/// A whole HTML document around `body`.
pub fn document(title: &str, body: &str) -> String {
    let title = title.replace('&', "&amp;").replace('<', "&lt;");
    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>{title}</title><style>{STYLE}</style></head>\n<body>\n{body}</body></html>\n"
    )
}

/// markdown-rs's four escapes undone.
fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
}

/// An SVG without its XML declaration.
fn bare(svg: &str) -> &str {
    svg.find("<svg").map_or(svg, |at| &svg[at..])
}

/// The line of the first diagram or formula with `source`.
fn line(tree: &Node, source: &str, diagram: bool) -> usize {
    let found = find(tree, source, diagram);
    if found.is_none() {
        log::error!(
            "mark: an undrawn {} has no line",
            if diagram { "diagram" } else { "formula" }
        );
    }
    found.unwrap_or(0)
}

fn find(tree: &Node, source: &str, diagram: bool) -> Option<usize> {
    let found = match tree {
        Node::Code(code) if diagram && code.lang.as_deref() == Some("mermaid") => {
            (code.value.trim_end() == source.trim_end()).then_some(tree)
        }
        Node::Math(math) if !diagram => {
            (math.value.trim_end() == source.trim_end()).then_some(tree)
        }
        Node::InlineMath(math) if !diagram => (math.value == source).then_some(tree),
        _ => None,
    };
    if let Some(at) = found.and_then(Node::position) {
        return Some(at.start.line);
    }
    tree.children()?
        .iter()
        .find_map(|child| find(child, source, diagram))
}
