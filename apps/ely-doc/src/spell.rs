//! Spelling suggestions from the kernel's context menu (DOC-12).

use ely_gpui_component::{
    menus::{ContextMenu, Menu, MenuItem},
    primitives::Severity,
};
use ely_lok::LokView;
use gpui::{Context, Entity, IntoElement, Pixels, Point};

use crate::docs::Docs;
use ely_office::prelude::*;

/// A misspelled word's menu, asked for at a point.
pub struct Offer {
    tab: u64,
    pub(crate) suggestions: Vec<(String, String)>,
    /// Bumps for each menu, so each opens once.
    request: (u64, Point<Pixels>),
}

impl Docs {
    /// Turns the kernel's context menu into a spelling offer.
    pub(crate) fn offer_spelling(
        &mut self,
        id: u64,
        view: &Entity<LokView>,
        cx: &mut Context<Self>,
    ) {
        let Some(json) = view.update(cx, |view, _| view.take_menu()) else {
            return;
        };
        let spelling = match ely_lok::review::spelling(&json) {
            Ok(spelling) => spelling,
            Err(error) => {
                let text = Docs::text(cx, "office.error.engine", &[("error", &error.to_string())]);
                return self.toast(text, Severity::Danger, cx);
            }
        };
        let Some(spelling) = spelling else {
            log::info!("doc: the kernel menu has no spelling");
            return;
        };
        let Some(caret) = view.read(cx).cursor_bounds() else {
            log::warn!("doc: a spelling menu without a caret");
            return;
        };
        let number = self.offer.as_ref().map_or(1, |offer| offer.request.0 + 1);
        log::info!("doc: {} spelling suggestions", spelling.suggestions.len());
        self.offer = Some(Offer {
            tab: id,
            suggestions: spelling.suggestions,
            request: (number, caret.bottom_left()),
        });
        cx.notify();
    }

    /// The spelling menu's host, for the active tab.
    pub(crate) fn spelling_menu(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let offer = self
            .offer
            .as_ref()
            .filter(|offer| Some(offer.tab) == self.core.active)?;
        let this = cx.entity();
        let mut menu = Menu::new();
        for (label, rule) in &offer.suggestions {
            let (this, rule) = (this.clone(), rule.clone());
            menu = menu.item(MenuItem::new(label.clone()).on_click(move |_, cx| {
                this.update(cx, |docs, cx| docs.run_active("spell.apply", &[&rule], cx))
            }));
        }
        if offer.suggestions.is_empty() {
            let none = Docs::text(cx, "doc.spell.none", &[]);
            menu = menu.item(MenuItem::new(none).disabled(true));
        }
        let ignore = Docs::text(cx, "doc.spell.ignore_all", &[]);
        let menu = menu
            .separator()
            .item(MenuItem::new(ignore).on_click(move |_, cx| {
                this.update(cx, |docs, cx| {
                    docs.run_active("spell.ignore_all", &["Spelling"], cx)
                })
            }));
        Some(ContextMenu::new("doc-spelling", menu).manual(Some(offer.request)))
    }
}
