//! Copy as Markdown and paste as plain text, as Typora's Edit menu.

use ely_gpui_component::primitives::Severity;
use ely_office::prelude::*;
use ely_ui::{commands, i18n::text};
use gpui::{App, ClipboardItem, Context, InteractiveElement as _, KeyBinding, actions};

use crate::desk::Desk;

actions!(mark, [CopyMarkdown, PastePlain]);

pub fn bind(cx: &mut App) {
    let at = Some(commands::CONTEXT);
    cx.bind_keys([
        KeyBinding::new("secondary-shift-c", CopyMarkdown, at),
        KeyBinding::new("secondary-shift-v", PastePlain, at),
    ]);
}

impl Desk {
    /// The selected Markdown, or the whole text, as written.
    fn copy_markdown(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.active_view() else {
            return self.no_document(cx);
        };
        let document = view.read(cx).document().read(cx);
        let selected = document.selected(cx);
        let markdown = match selected.is_empty() {
            true => document.text(cx),
            false => selected,
        };
        log::info!("mark: {} bytes copied as Markdown", markdown.len());
        cx.write_to_clipboard(ClipboardItem::new_string(markdown));
    }

    /// The clipboard's text at the caret, pictures left out.
    fn paste_plain(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.active_view() else {
            return self.no_document(cx);
        };
        let Some(pasted) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            let said = text(cx, "mark.paste.none", &[]);
            return self.toast(said, Severity::Info, cx);
        };
        let document = view.read(cx).document().clone();
        if !document.update(cx, |document, cx| document.insert(&pasted, cx)) {
            let said = text(cx, "mark.preview.read_only", &[]);
            self.toast(said, Severity::Info, cx);
        }
    }

    pub fn clip_actions(root: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        root.on_action(cx.listener(|desk, _: &CopyMarkdown, _, cx| desk.copy_markdown(cx)))
            .on_action(cx.listener(|desk, _: &PastePlain, _, cx| desk.paste_plain(cx)))
    }
}
