//! The right-click menu, laid out as Word's (DOC-02, DOC-07, DOC-12).

use ely_gpui_component::{
    menus::{ContextMenu, Menu, MenuItem},
    primitives::{IconName, Severity},
};
use ely_lok::{
    LokView,
    menu::{Offered, Target},
};
use gpui::{App, Context, Entity, Pixels, Point, Window};

use crate::docs::Docs;
use crate::refs::{InsertCaption, InsertComment, InsertLink};
use ely_office::prelude::*;

/// A menu the kernel offered, asked for at a point.
pub struct Offer {
    tab: u64,
    pub(crate) offered: Offered,
    /// Bumps for each menu, so each opens once.
    request: (u64, Point<Pixels>),
}

/// One thing the menu does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pick {
    Cut,
    Copy,
    Paste,
    Link,
    Comment,
    ColumnLeft,
    ColumnRight,
    RowAbove,
    RowBelow,
    DeleteRows,
    DeleteColumns,
    DeleteTable,
    Merge,
    SplitColumns,
    SplitRows,
    EvenRows,
    EvenColumns,
    Inline,
    Square,
    TopBottom,
    InFront,
    Front,
    Forward,
    Back,
    Backward,
    Caption,
}

/// A row, a submenu or a line.
enum Entry {
    Row(Pick),
    Sub(&'static str, &'static [Pick]),
    Line,
}

const TABLE: &[Entry] = &[
    Entry::Sub(
        "doc.menu.insert",
        &[Pick::ColumnLeft, Pick::ColumnRight, Pick::RowAbove, Pick::RowBelow],
    ),
    Entry::Sub(
        "doc.menu.delete",
        &[Pick::DeleteRows, Pick::DeleteColumns, Pick::DeleteTable],
    ),
    Entry::Row(Pick::Merge),
    Entry::Sub("doc.menu.split", &[Pick::SplitColumns, Pick::SplitRows]),
    Entry::Row(Pick::EvenRows),
    Entry::Row(Pick::EvenColumns),
    Entry::Line,
    Entry::Row(Pick::Link),
    Entry::Row(Pick::Comment),
];

const FRAME: &[Entry] = &[
    Entry::Sub(
        "doc.menu.wrap",
        &[Pick::Inline, Pick::Square, Pick::TopBottom, Pick::InFront],
    ),
    Entry::Sub("doc.menu.front", &[Pick::Front, Pick::Forward]),
    Entry::Sub("doc.menu.back", &[Pick::Back, Pick::Backward]),
    Entry::Line,
    Entry::Row(Pick::Caption),
    Entry::Row(Pick::Comment),
];

const TEXT: &[Entry] = &[Entry::Row(Pick::Link), Entry::Row(Pick::Comment)];

impl Pick {
    /// Its label key and the kernel command that gates it.
    fn row(self) -> (&'static str, &'static str) {
        match self {
            Pick::Cut => ("doc.menu.cut", ".uno:Cut"),
            Pick::Copy => ("doc.menu.copy", ".uno:Copy"),
            Pick::Paste => ("doc.menu.paste", ".uno:Paste"),
            Pick::Link => ("doc.menu.link", ".uno:InsertHyperlink"),
            Pick::Comment => ("doc.menu.comment", ".uno:InsertAnnotation"),
            Pick::ColumnLeft => ("doc.menu.column_left", ".uno:InsertColumnsBefore"),
            Pick::ColumnRight => ("doc.menu.column_right", ".uno:InsertColumnsAfter"),
            Pick::RowAbove => ("doc.menu.row_above", ".uno:InsertRowsBefore"),
            Pick::RowBelow => ("doc.menu.row_below", ".uno:InsertRowsAfter"),
            Pick::DeleteRows => ("doc.menu.delete_rows", ".uno:DeleteRows"),
            Pick::DeleteColumns => ("doc.menu.delete_columns", ".uno:DeleteColumns"),
            Pick::DeleteTable => ("doc.menu.delete_table", ".uno:DeleteTable"),
            Pick::Merge => ("doc.menu.merge", ".uno:MergeCells"),
            Pick::SplitColumns => ("doc.menu.split_columns", ".uno:SplitCell"),
            Pick::SplitRows => ("doc.menu.split_rows", ".uno:SplitCell"),
            Pick::EvenRows => ("doc.menu.even_rows", ".uno:DistributeRows"),
            Pick::EvenColumns => ("doc.menu.even_columns", ".uno:DistributeColumns"),
            Pick::Inline => ("doc.menu.inline", ".uno:SetAnchorToChar"),
            Pick::Square => ("doc.menu.square", ".uno:WrapOn"),
            Pick::TopBottom => ("doc.menu.top_bottom", ".uno:WrapOff"),
            Pick::InFront => ("doc.menu.in_front", ".uno:WrapThrough"),
            Pick::Front => ("doc.menu.front", ".uno:BringToFront"),
            Pick::Forward => ("doc.menu.forward", ".uno:ObjectForwardOne"),
            Pick::Back => ("doc.menu.back", ".uno:SendToBack"),
            Pick::Backward => ("doc.menu.backward", ".uno:ObjectBackOne"),
            Pick::Caption => ("doc.menu.caption", ".uno:InsertCaptionDialog"),
        }
    }

    /// The kernel offers it and it runs now.
    pub(crate) fn on(self, offered: &Offered) -> bool {
        offered.allows(self.row().1)
    }
}

impl Docs {
    /// Turns the kernel's context menu into Ely's.
    pub(crate) fn offer_menu(&mut self, id: u64, view: &Entity<LokView>, cx: &mut Context<Self>) {
        let Some((json, at)) = view.update(cx, |view, _| view.take_menu()) else {
            return;
        };
        let offered = match ely_lok::menu::read(&json) {
            Ok(offered) => offered,
            Err(error) => {
                let text = Docs::text(cx, "office.error.engine", &[("error", &error.to_string())]);
                return self.toast(text, Severity::Danger, cx);
            }
        };
        // Without a click it opens at the caret.
        let caret = || view.read(cx).cursor_bounds().map(|caret| caret.bottom_left());
        let Some(at) = at.or_else(caret) else {
            log::warn!("doc: a menu with no click and no caret");
            return;
        };
        let number = self.offer.as_ref().map_or(1, |offer| offer.request.0 + 1);
        let suggestions = offered.spelling.as_ref().map_or(0, |spelling| spelling.suggestions.len());
        log::info!("doc: a {:?} menu, {suggestions} suggestions", offered.target);
        self.offer = Some(Offer {
            tab: id,
            offered,
            request: (number, at),
        });
        cx.notify();
    }

    /// The menu's host, for the active tab.
    pub(crate) fn context_menu(&self, cx: &mut Context<Self>) -> Option<ContextMenu> {
        let offer = self
            .offer
            .as_ref()
            .filter(|offer| Some(offer.tab) == self.core.active)?;
        let menu = menu_of(&cx.entity(), &offer.offered, cx);
        Some(ContextMenu::new("doc-menu", menu).manual(Some(offer.request)))
    }

    /// Runs one menu row.
    pub(crate) fn choose(&mut self, pick: Pick, window: &mut Window, cx: &mut Context<Self>) {
        log::info!("doc: the menu picks {pick:?}");
        let (id, values): (&str, &[&str]) = match pick {
            Pick::Cut | Pick::Copy | Pick::Paste => return self.clipboard(pick, cx),
            Pick::Link => return self.on_link(&InsertLink, window, cx),
            Pick::Comment => return self.on_comment(&InsertComment, window, cx),
            Pick::Caption => return self.on_caption(&InsertCaption, window, cx),
            Pick::ColumnLeft => ("table.column_left", &[]),
            Pick::ColumnRight => ("table.column_right", &[]),
            Pick::RowAbove => ("table.row_above", &[]),
            Pick::RowBelow => ("table.row_below", &[]),
            Pick::DeleteRows => ("table.delete_rows", &[]),
            Pick::DeleteColumns => ("table.delete_columns", &[]),
            Pick::DeleteTable => ("table.delete", &[]),
            Pick::Merge => ("table.merge", &[]),
            Pick::SplitColumns => ("table.split", &["2", "false"]),
            Pick::SplitRows => ("table.split", &["2", "true"]),
            Pick::EvenRows => ("table.even_rows", &[]),
            Pick::EvenColumns => ("table.even_columns", &[]),
            Pick::Inline => ("image.anchor_as_char", &[]),
            Pick::Square => ("image.wrap_on", &[]),
            Pick::TopBottom => ("image.wrap_off", &[]),
            Pick::InFront => ("image.wrap_through", &[]),
            Pick::Front => ("image.front", &[]),
            Pick::Forward => ("image.forward", &[]),
            Pick::Back => ("image.back", &[]),
            Pick::Backward => ("image.backward", &[]),
        };
        self.run_active(id, values, cx);
    }

    fn clipboard(&mut self, pick: Pick, cx: &mut Context<Self>) {
        let Some(view) = self.active_tab().map(|tab| tab.view.clone()) else {
            log::warn!("doc: {pick:?} with no document");
            return;
        };
        view.update(cx, |view, _| match pick {
            Pick::Paste => view.paste(),
            _ => view.copy(pick == Pick::Cut),
        });
    }
}

/// Spelling first, then the clipboard row, then the target's rows.
pub(crate) fn menu_of(this: &Entity<Docs>, offered: &Offered, cx: &App) -> Menu {
    let item = |pick: Pick| {
        let this = this.clone();
        MenuItem::new(Docs::text(cx, pick.row().0, &[]))
            .disabled(!pick.on(offered))
            .on_click(move |window, cx| this.update(cx, |docs, cx| docs.choose(pick, window, cx)))
    };
    let mut menu = Menu::new();
    if let Some(spelling) = &offered.spelling {
        for (label, rule) in &spelling.suggestions {
            let (this, rule) = (this.clone(), rule.clone());
            menu = menu.item(MenuItem::new(label.clone()).on_click(move |_, cx| {
                this.update(cx, |docs, cx| docs.run_active("spell.apply", &[&rule], cx))
            }));
        }
        if spelling.suggestions.is_empty() {
            menu = menu.item(MenuItem::new(Docs::text(cx, "doc.spell.none", &[])).disabled(true));
        }
        let this = this.clone();
        let ignore = MenuItem::new(Docs::text(cx, "doc.spell.ignore_all", &[])).on_click(
            move |_, cx| {
                this.update(cx, |docs, cx| {
                    docs.run_active("spell.ignore_all", &["Spelling"], cx)
                })
            },
        );
        menu = menu.item(ignore).separator();
    }
    let clipboard = [
        (Pick::Cut, IconName::Scissors),
        (Pick::Copy, IconName::Copy),
        (Pick::Paste, IconName::Clipboard),
    ];
    menu = menu
        .buttons(clipboard.map(|(pick, icon)| item(pick).icon(icon)))
        .separator();
    let entries = match offered.target {
        Target::Table => TABLE,
        Target::Frame => FRAME,
        Target::Text => TEXT,
    };
    for entry in entries {
        menu = match entry {
            Entry::Row(pick) => menu.item(item(*pick)),
            Entry::Sub(key, picks) => {
                let sub = picks.iter().fold(Menu::new(), |sub, pick| sub.item(item(*pick)));
                let on = picks.iter().any(|pick| pick.on(offered));
                menu.item(MenuItem::submenu(Docs::text(cx, key, &[]), sub).disabled(!on))
            }
            Entry::Line => menu.separator(),
        };
    }
    menu
}

#[cfg(test)]
mod tests;
