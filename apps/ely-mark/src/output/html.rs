//! A syntax tree written as HTML, node by node.

use std::{collections::HashMap, io};

use ely_gpui_component::{documents::diagram_svg, typography::formula_svg};
use markdown::mdast::{AlignKind, Node};

use super::{Page, attribute, escape, pictures::Placed};

/// Writes `tree` into `page`; pictures go through `placed`.
pub(super) fn write(tree: &Node, placed: &mut Placed, page: &mut Page) -> io::Result<()> {
    let mut writer = Writer {
        placed,
        page,
        links: HashMap::new(),
        notes: HashMap::new(),
        called: Vec::new(),
    };
    writer.gather(tree);
    writer.children(tree, false)?;
    writer.footnotes()
}

struct Writer<'a, 'b, 'c> {
    placed: &'b mut Placed<'a>,
    page: &'c mut Page,
    /// Link definitions by identifier: address and title.
    links: HashMap<String, (String, Option<String>)>,
    notes: HashMap<String, Node>,
    /// Footnotes in the order first called.
    called: Vec<String>,
}

impl Writer<'_, '_, '_> {
    fn gather(&mut self, node: &Node) {
        match node {
            Node::Definition(definition) => {
                let target = (definition.url.clone(), definition.title.clone());
                self.links
                    .entry(definition.identifier.clone())
                    .or_insert(target);
            }
            Node::FootnoteDefinition(note) => {
                self.notes
                    .entry(note.identifier.clone())
                    .or_insert_with(|| node.clone());
            }
            _ => {}
        }
        for child in node.children().into_iter().flatten() {
            self.gather(child);
        }
    }

    fn push(&mut self, text: &str) {
        self.page.html.push_str(text);
    }

    fn children(&mut self, node: &Node, tight: bool) -> io::Result<()> {
        for child in node.children().into_iter().flatten() {
            self.node(child, tight)?;
        }
        Ok(())
    }

    fn wrapped(&mut self, tag: &str, node: &Node) -> io::Result<()> {
        self.push(&format!("<{tag}>"));
        self.children(node, false)?;
        self.push(&format!("</{tag}>"));
        Ok(())
    }

    fn node(&mut self, node: &Node, tight: bool) -> io::Result<()> {
        match node {
            Node::Paragraph(_) if tight => self.children(node, false)?,
            Node::Paragraph(_) => self.wrapped("p", node)?,
            Node::Heading(heading) => self.wrapped(&format!("h{}", heading.depth), node)?,
            Node::Blockquote(_) => self.wrapped("blockquote", node)?,
            Node::Emphasis(_) => self.wrapped("em", node)?,
            Node::Strong(_) => self.wrapped("strong", node)?,
            Node::Delete(_) => self.wrapped("del", node)?,
            Node::List(list) => {
                let tag = if list.ordered { "ol" } else { "ul" };
                match list.start.filter(|start| list.ordered && *start != 1) {
                    Some(start) => self.push(&format!("<ol start=\"{start}\">")),
                    None => self.push(&format!("<{tag}>")),
                }
                for item in &list.children {
                    let Node::ListItem(each) = item else { continue };
                    self.push("<li>");
                    if let Some(checked) = each.checked {
                        let mark = if checked { " checked" } else { "" };
                        self.push(&format!("<input type=\"checkbox\" disabled{mark}> "));
                    }
                    self.children(item, !list.spread && !each.spread)?;
                    self.push("</li>");
                }
                self.push(&format!("</{tag}>"));
            }
            Node::Table(table) => self.table(node, &table.align)?,
            Node::Code(code) if code.lang.as_deref() == Some("mermaid") => {
                let line = line_of(node);
                match diagram_svg(&code.value) {
                    Ok(svg) => self.push(&format!("<div class=\"diagram\">{}</div>", bare(&svg))),
                    Err(_) => {
                        self.page.diagrams.push(line);
                        self.code(Some("mermaid"), &code.value);
                    }
                }
            }
            Node::Code(code) => self.code(code.lang.as_deref(), &code.value),
            Node::Math(math) => self.math(node, &math.value, true),
            Node::InlineMath(math) => self.math(node, &math.value, false),
            Node::InlineCode(code) => self.push(&format!("<code>{}</code>", escape(&code.value))),
            Node::Text(text) => self.push(&escape(&text.value)),
            // HTML in the document shows as text, never runs.
            Node::Html(html) => self.push(&escape(&html.value)),
            Node::Break(_) => self.push("<br>"),
            Node::ThematicBreak(_) => self.push("<hr>"),
            Node::Link(link) => self.link(node, &link.url, link.title.as_deref())?,
            Node::LinkReference(link) => match self.links.get(&link.identifier).cloned() {
                Some((url, title)) => self.link(node, &url, title.as_deref())?,
                None => self.children(node, false)?,
            },
            Node::Image(image) => self.picture(&image.url, &image.alt, image.title.as_deref())?,
            Node::ImageReference(image) => match self.links.get(&image.identifier).cloned() {
                Some((url, title)) => self.picture(&url, &image.alt, title.as_deref())?,
                None => self.push(&escape(&image.alt)),
            },
            Node::FootnoteReference(call) => {
                let number = match self.called.iter().position(|id| *id == call.identifier) {
                    Some(at) => at + 1,
                    None => {
                        self.called.push(call.identifier.clone());
                        self.called.len()
                    }
                };
                self.push(&format!("<sup><a href=\"#fn-{number}\">{number}</a></sup>"));
            }
            // Written elsewhere, or never.
            Node::Yaml(_) | Node::Toml(_) | Node::Definition(_) | Node::FootnoteDefinition(_) => {}
            _ => self.children(node, false)?,
        }
        Ok(())
    }

    fn table(&mut self, node: &Node, align: &[AlignKind]) -> io::Result<()> {
        self.push("<table>");
        for (at, row) in node.children().into_iter().flatten().enumerate() {
            let cell = if at == 0 { "th" } else { "td" };
            self.push("<tr>");
            for (column, each) in row.children().into_iter().flatten().enumerate() {
                let side = match align.get(column) {
                    Some(AlignKind::Left) => " style=\"text-align:left\"",
                    Some(AlignKind::Right) => " style=\"text-align:right\"",
                    Some(AlignKind::Center) => " style=\"text-align:center\"",
                    _ => "",
                };
                self.push(&format!("<{cell}{side}>"));
                self.children(each, false)?;
                self.push(&format!("</{cell}>"));
            }
            self.push("</tr>");
        }
        self.push("</table>");
        Ok(())
    }

    fn code(&mut self, lang: Option<&str>, value: &str) {
        let class = lang.map_or_else(String::new, |lang| {
            format!(" class=\"language-{}\"", attribute(lang))
        });
        self.push(&format!("<pre><code{class}>{}</code></pre>", escape(value)));
    }

    fn math(&mut self, node: &Node, value: &str, display: bool) {
        match (formula_svg(value, display), display) {
            (Ok(svg), true) => self.push(&format!("<div class=\"math\">{}</div>", bare(&svg))),
            (Ok(svg), false) => self.push(&format!("<span class=\"math\">{}</span>", bare(&svg))),
            (Err(error), _) => {
                self.page.formulas.push((line_of(node), error.to_string()));
                match display {
                    true => self.code(Some("math"), value),
                    false => self.push(&format!("<code>{}</code>", escape(value))),
                }
            }
        }
    }

    fn link(&mut self, node: &Node, url: &str, title: Option<&str>) -> io::Result<()> {
        let title = title.map_or_else(String::new, |title| {
            format!(" title=\"{}\"", attribute(title))
        });
        // Only web, mail and in-page addresses follow.
        match safe(url) {
            true => self.push(&format!("<a href=\"{}\"{title}>", attribute(url))),
            false => self.push("<a>"),
        }
        self.children(node, false)?;
        self.push("</a>");
        Ok(())
    }

    fn picture(&mut self, url: &str, alt: &str, title: Option<&str>) -> io::Result<()> {
        let src = self.placed.place(url)?;
        if src.is_none() {
            self.page.missing.push(if url.is_empty() {
                alt.to_owned()
            } else {
                url.to_owned()
            });
        }
        // A PDF shows what it left out as words.
        let Some(src) = src.or_else(|| (!self.placed.pdf()).then(|| url.to_owned())) else {
            self.push(&format!("[{}]", escape(alt)));
            return Ok(());
        };
        let title = title.map_or_else(String::new, |title| {
            format!(" title=\"{}\"", attribute(title))
        });
        self.push(&format!(
            "<img src=\"{}\" alt=\"{}\"{title}>",
            attribute(&src),
            attribute(alt)
        ));
        Ok(())
    }

    fn footnotes(&mut self) -> io::Result<()> {
        if self.called.is_empty() {
            return Ok(());
        }
        self.push("<section class=\"footnotes\"><hr><ol>");
        // A note may call another note.
        let mut at = 0;
        while at < self.called.len() {
            let note = self.notes.get(&self.called[at]).cloned();
            at += 1;
            self.push(&format!("<li id=\"fn-{at}\">"));
            match note {
                Some(note) => self.children(&note, false)?,
                None => log::error!("mark: footnote {at} has no definition"),
            }
            self.push("</li>");
        }
        self.push("</ol></section>");
        Ok(())
    }
}

/// The line a node starts on, from one.
fn line_of(node: &Node) -> usize {
    node.position().map_or(0, |at| at.start.line)
}

/// An SVG without its XML declaration.
fn bare(svg: &str) -> &str {
    svg.find("<svg").map_or(svg, |at| &svg[at..])
}

/// A plain, web or mail address, as a browser reads it.
fn safe(url: &str) -> bool {
    // Browsers trim spaces and controls, then drop tabs and newlines.
    let read: String = url
        .trim_matches(|ch: char| ch <= ' ')
        .chars()
        .filter(|ch| !matches!(ch, '\t' | '\n' | '\r'))
        .collect();
    match scheme(&read) {
        None => true,
        Some(scheme) => ["http", "https", "mailto"].contains(&scheme.to_lowercase().as_str()),
    }
}

/// An address's scheme, when it has one.
pub(super) fn scheme(url: &str) -> Option<&str> {
    let end = url.find(':')?;
    let name = &url[..end];
    let plain = name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "+-.".contains(ch));
    // A drive letter is a path, not a scheme.
    (plain && name.len() > 1).then_some(name)
}
