//! Markdown as plain text: words, code and TeX, no marks.

use std::io;

use ely_gpui_component::documents::markdown_options;
use markdown::mdast::Node;

/// `text` as plain text, a blank line between blocks.
pub fn plain(text: &str) -> io::Result<String> {
    let tree = markdown::to_mdast(text, &markdown_options())
        .map_err(|error| io::Error::other(error.to_string()))?;
    let mut blocks = Vec::new();
    for child in tree.children().into_iter().flatten() {
        block(child, &mut blocks);
    }
    Ok(blocks.join("\n\n") + "\n")
}

fn block(node: &Node, blocks: &mut Vec<String>) {
    match node {
        Node::Code(code) => blocks.push(code.value.clone()),
        Node::Math(math) => blocks.push(math.value.clone()),
        Node::List(list) => {
            let items = list.children.iter().map(|item| {
                let mut inner = Vec::new();
                for child in item.children().into_iter().flatten() {
                    block(child, &mut inner);
                }
                inner.join("\n")
            });
            blocks.push(items.collect::<Vec<_>>().join("\n"));
        }
        Node::Table(table) => {
            let rows = table.children.iter().map(|row| {
                let cells = row.children().into_iter().flatten().map(words);
                cells.collect::<Vec<_>>().join("\t")
            });
            blocks.push(rows.collect::<Vec<_>>().join("\n"));
        }
        Node::Blockquote(_) | Node::FootnoteDefinition(_) => {
            for child in node.children().into_iter().flatten() {
                block(child, blocks);
            }
        }
        Node::Paragraph(_) | Node::Heading(_) => blocks.push(words(node)),
        // Front matter, HTML, rules and definitions are not words.
        _ => {}
    }
}

/// The words of an inline run.
fn words(node: &Node) -> String {
    match node {
        Node::Text(text) => text.value.clone(),
        Node::InlineCode(code) => code.value.clone(),
        Node::InlineMath(math) => math.value.clone(),
        Node::Break(_) => "\n".into(),
        Node::Image(image) => image.alt.clone(),
        Node::Html(_) => String::new(),
        _ => node.children().into_iter().flatten().map(words).collect(),
    }
}
