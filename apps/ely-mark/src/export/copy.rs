//! Rich, HTML code and plain text copies (R4.10).

use std::path::PathBuf;

use ely_gpui_component::primitives::Severity;
use ely_office::prelude::*;
use ely_platform::clipboard::{self, Clip};
use ely_ui::i18n::text;
use gpui::{ClipboardItem, Context, Div, InteractiveElement as _, actions};

use crate::desk::Desk;
use crate::output::{self, Pictures};

actions!(mark, [CopyRich, CopyHtml, CopyPlain]);

impl Desk {
    pub(super) fn copy_actions(root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(cx.listener(|desk, _: &CopyRich, _, cx| desk.copy_rich(cx)))
            .on_action(cx.listener(|desk, _: &CopyHtml, _, cx| desk.copy_html(cx)))
            .on_action(cx.listener(|desk, _: &CopyPlain, _, cx| desk.copy_plain(cx)))
    }

    /// The selection or whole text, and the file's folder.
    fn copied_text(&mut self, cx: &mut Context<Self>) -> Option<(String, Option<PathBuf>)> {
        let Some(view) = self.active_view() else {
            self.no_document(cx);
            return None;
        };
        let document = view.read(cx).document().read(cx);
        let selected = document.selected(cx);
        let markdown = match selected.is_empty() {
            true => document.text(cx),
            false => selected,
        };
        let beside = (self.active_tab())
            .and_then(|tab| tab.session.path())
            .and_then(|path| path.parent())
            .map(PathBuf::from);
        Some((markdown, beside))
    }

    fn copy_failed(&mut self, error: &std::io::Error, cx: &mut Context<Self>) {
        let said = text(cx, "mark.copy.failed", &[("error", &error.to_string())]);
        self.toast(said, Severity::Danger, cx);
    }

    /// HTML for pasting, the Markdown beside it.
    fn copy_rich(&mut self, cx: &mut Context<Self>) {
        let Some((markdown, beside)) = self.copied_text(cx) else {
            return;
        };
        let copied =
            output::body(&markdown, beside.as_deref(), &Pictures::Inline).and_then(|mut page| {
                let clip = Clip::Html {
                    html: std::mem::take(&mut page.html),
                    text: markdown,
                };
                clipboard::write(&clip)
                    .map(|()| page)
                    .map_err(|error| std::io::Error::other(format!("{error:#}")))
            });
        match copied {
            Ok(page) => self.left_out(&page, cx),
            Err(error) => self.copy_failed(&error, cx),
        }
    }

    /// The HTML as text; pictures keep their addresses.
    fn copy_html(&mut self, cx: &mut Context<Self>) {
        let Some((markdown, beside)) = self.copied_text(cx) else {
            return;
        };
        match output::body(&markdown, beside.as_deref(), &Pictures::Written) {
            Ok(mut page) => {
                log::info!("mark: {} bytes copied as HTML code", page.html.len());
                cx.write_to_clipboard(ClipboardItem::new_string(std::mem::take(&mut page.html)));
                self.left_out(&page, cx);
            }
            Err(error) => self.copy_failed(&error, cx),
        }
    }

    fn copy_plain(&mut self, cx: &mut Context<Self>) {
        let Some((markdown, _)) = self.copied_text(cx) else {
            return;
        };
        match output::plain(&markdown) {
            Ok(plain) => {
                log::info!("mark: {} bytes copied as plain text", plain.len());
                cx.write_to_clipboard(ClipboardItem::new_string(plain));
            }
            Err(error) => self.copy_failed(&error, cx),
        }
    }
}
