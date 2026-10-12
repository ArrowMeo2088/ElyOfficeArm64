//! The table menu's work on the shown file (MARK-06).

use ely_gpui_component::{documents::TableEdit, primitives::Severity};
use ely_office::prelude::*;
use ely_ui::i18n::text;
use gpui::{Context, InteractiveElement as _, actions};

use crate::desk::Desk;

actions!(
    mark,
    [
        InsertTable,
        RowAbove,
        RowBelow,
        ColumnLeft,
        ColumnRight,
        DeleteRow,
        DeleteColumn
    ]
);

impl Desk {
    fn table(&mut self, edit: Option<TableEdit>, cx: &mut Context<Self>) {
        let Some(view) = self.active_view() else {
            let said = text(cx, "command.blocked.no_document", &[]);
            return self.toast(said, Severity::Warning, cx);
        };
        let document = view.read(cx).document().clone();
        let done = document.update(cx, |document, cx| match edit {
            Some(edit) => document.edit_table(edit, cx),
            None => {
                document.insert_table(cx);
                true
            }
        });
        if !done {
            let said = text(cx, "mark.table.none", &[]);
            self.toast(said, Severity::Info, cx);
        }
    }

    pub fn table_actions(element: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        element
            .on_action(cx.listener(|desk, _: &InsertTable, _, cx| desk.table(None, cx)))
            .on_action(
                cx.listener(|desk, _: &RowAbove, _, cx| desk.table(Some(TableEdit::RowAbove), cx)),
            )
            .on_action(
                cx.listener(|desk, _: &RowBelow, _, cx| desk.table(Some(TableEdit::RowBelow), cx)),
            )
            .on_action(cx.listener(|desk, _: &ColumnLeft, _, cx| {
                desk.table(Some(TableEdit::ColumnLeft), cx)
            }))
            .on_action(cx.listener(|desk, _: &ColumnRight, _, cx| {
                desk.table(Some(TableEdit::ColumnRight), cx)
            }))
            .on_action(
                cx.listener(|desk, _: &DeleteRow, _, cx| {
                    desk.table(Some(TableEdit::DeleteRow), cx)
                }),
            )
            .on_action(cx.listener(|desk, _: &DeleteColumn, _, cx| {
                desk.table(Some(TableEdit::DeleteColumn), cx)
            }))
    }
}
