//! Mail merge from a CSV (DOC-14); never sends mail.

mod run;

use ely_gpui_component::{
    buttons::{Button, ButtonVariant, SegmentedControl},
    data_display::{PropertyGrid, PropertyGroup},
    forms::NumberInput,
    layout::ScrollArea,
    primitives::Severity,
    theme::ControlSize,
};
use gpui::{
    Context, IntoElement, ParentElement as _, Render, Styled as _, WeakEntity, Window, actions, div,
};

use crate::docs::{Docs, Side};
use ely_office::prelude::*;
pub use run::{Table, marker, read_csv};

actions!(doc, [MailMerge]);

/// ElyDoc's Tools menu.
pub fn menu(cx: &gpui::App) -> gpui::Menu {
    let label = |key: &str| Docs::text(cx, key, &[]);
    gpui::Menu::new(label("menu.tools")).items([
        gpui::MenuItem::action(label("office.find.open"), ely_office::FindReplace),
        gpui::MenuItem::action(label("doc.merge.open"), MailMerge),
        gpui::MenuItem::separator(),
        gpui::MenuItem::action(label("doc.nav.open"), crate::navigate::ToggleNavigator),
        gpui::MenuItem::action(label("doc.nav.back"), crate::navigate::GoBack),
    ])
}

/// The merge panel: the data, the record shown, the format.
pub struct Merge {
    docs: WeakEntity<Docs>,
    pub table: Option<Table>,
    /// The record to preview, from 1.
    record: usize,
    pdf: bool,
}

impl Merge {
    pub fn new(docs: WeakEntity<Docs>) -> Self {
        Merge {
            docs,
            table: None,
            record: 1,
            pdf: false,
        }
    }
}

impl Docs {
    pub fn on_merge(&mut self, _: &MailMerge, _: &mut Window, cx: &mut Context<Self>) {
        self.side = Side::Merge;
        cx.notify();
        if self.merge.read(cx).table.is_none() {
            self.choose_csv(cx);
        }
    }

    pub(crate) fn choose_csv(&mut self, cx: &mut Context<Self>) {
        self.pick(false, cx, |docs, path, cx| match read_csv(&path) {
            Ok(table) => {
                log::info!("doc: merge data has {} rows", table.rows.len());
                docs.merge.update(cx, |merge, cx| {
                    merge.table = Some(table);
                    merge.record = 1;
                    cx.notify();
                });
            }
            Err(error) => {
                let name = path.display().to_string();
                let error = error.to_string();
                let text = Docs::text(
                    cx,
                    "doc.merge.unreadable",
                    &[("name", &name), ("error", &error)],
                );
                docs.toast(text, Severity::Danger, cx);
            }
        });
    }

    /// The tab a merge starts from: the active one.
    fn source(&self, cx: &mut Context<Self>) -> Option<u64> {
        if self.core.active.is_none() {
            let text = Docs::text(cx, "command.blocked.no_document", &[]);
            self.toast(text, Severity::Warning, cx);
        }
        self.core.active
    }

    /// Types a field's marker at the cursor.
    pub(crate) fn insert_field(&mut self, header: &str, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else {
            let text = Docs::text(cx, "command.blocked.no_document", &[]);
            return self.toast(text, Severity::Warning, cx);
        };
        tab.view.update(cx, |view, _| view.insert(&marker(header)));
    }
}

impl Render for Merge {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (docs, this) = (self.docs.clone(), cx.entity());
        let text = |key: &str| Docs::text(cx, key, &[]);
        let choose = {
            let docs = docs.clone();
            Button::new("doc-merge-choose", text("doc.merge.choose"))
                .size(ControlSize::Sm)
                .on_click(move |_, _, cx| {
                    docs.update(cx, |docs, cx| docs.choose_csv(cx))
                        .expect("the documents live");
                })
        };
        let Some(table) = self.table.clone() else {
            let group =
                PropertyGroup::new(text("doc.merge.title")).row(text("doc.merge.data"), choose);
            return PropertyGrid::new("doc-merge")
                .group(group)
                .into_any_element();
        };
        let fields = table
            .headers
            .iter()
            .enumerate()
            .map(|(at, header)| {
                let (docs, header) = (docs.clone(), header.clone());
                Button::new(("doc-merge-field", at), header.clone())
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .on_click(move |_, _, cx| {
                        docs.update(cx, |docs, cx| docs.insert_field(&header, cx))
                            .expect("the documents live");
                    })
            })
            .collect::<Vec<_>>();
        let record = {
            let this = this.clone();
            NumberInput::new("doc-merge-record", self.record as f64)
                .range(1.0, table.rows.len() as f64)
                .on_commit(move |value, _, cx| {
                    this.update(cx, |merge, cx| {
                        merge.record = value.round() as usize;
                        cx.notify();
                    })
                })
        };
        let preview = {
            let (docs, table, at) = (docs.clone(), table.clone(), self.record - 1);
            Button::new("doc-merge-preview", text("doc.merge.preview"))
                .size(ControlSize::Sm)
                .on_click(move |_, _, cx| {
                    docs.update(cx, |docs, cx| {
                        if let Some(source) = docs.source(cx) {
                            docs.preview_record(source, &table, at, cx)
                        }
                    })
                    .expect("the documents live");
                })
        };
        let format = {
            let this = this.clone();
            SegmentedControl::new("doc-merge-format", if self.pdf { "pdf" } else { "docx" })
                .size(ControlSize::Sm)
                .segment("docx", "DOCX", None)
                .segment("pdf", "PDF", None)
                .on_change(move |value, _, cx| {
                    this.update(cx, |merge, cx| {
                        merge.pdf = value.as_ref() == "pdf";
                        cx.notify();
                    })
                })
        };
        let generate = {
            let (pdf, table) = (self.pdf, table.clone());
            Button::new("doc-merge-generate", text("doc.merge.generate"))
                .size(ControlSize::Sm)
                .variant(ButtonVariant::Primary)
                .on_click(move |_, _, cx| {
                    let table = table.clone();
                    docs.update(cx, |docs, cx| {
                        let Some(source) = docs.source(cx) else {
                            return;
                        };
                        docs.pick(true, cx, move |docs, folder, cx| {
                            docs.generate(source, table, pdf, folder, cx)
                        })
                    })
                    .expect("the documents live");
                })
        };
        let row = || div().flex().flex_wrap().items_center().gap_1();
        let group = PropertyGroup::new(table.name.clone())
            .row(text("doc.merge.data"), choose)
            .row(text("doc.merge.fields"), row().children(fields))
            .row(
                text("doc.merge.record"),
                row().child(div().w_full().max_w_24().child(record)).child(preview),
            )
            .row(
                text("doc.merge.output"),
                row().child(format).child(generate),
            );
        ScrollArea::new("doc-merge-scroll").vertical()
            .size_full()
            .child(PropertyGrid::new("doc-merge").group(group))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests;
