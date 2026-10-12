//! Modes, menus and keys (MARK-01, MARK-02).

use ely_ui::{commands, i18n::text};
use gpui::{App, KeyBinding, Menu, MenuItem, actions};

use crate::view::Mode;

actions!(mark, [VisualMode, SourceMode, PreviewMode, CheckDisk]);

pub fn bind(cx: &mut App) {
    let at = Some(commands::CONTEXT);
    cx.bind_keys([
        KeyBinding::new("secondary-1", VisualMode, at),
        KeyBinding::new("secondary-2", SourceMode, at),
        KeyBinding::new("secondary-3", PreviewMode, at),
        KeyBinding::new("secondary-shift-f", crate::look::FocusMode, at),
        KeyBinding::new("secondary-=", crate::look::Larger, at),
        KeyBinding::new("secondary--", crate::look::Smaller, at),
        KeyBinding::new("secondary-0", crate::look::ActualSize, at),
        KeyBinding::new("secondary-f", ely_office::FindReplace, at),
    ]);
}

/// The label of `mode`.
pub fn label(mode: Mode, cx: &App) -> gpui::SharedString {
    text(cx, &format!("mark.mode.{}", mode.name()), &[])
}

pub fn menus(look: &crate::look::Look, cx: &App) -> Vec<Menu> {
    use crate::look::{self, Font, Width};
    let checked = |key: &str, action: Box<dyn gpui::Action>, checked: bool| MenuItem::Action {
        name: text(cx, key, &[]),
        action,
        os_action: None,
        checked,
        disabled: false,
    };
    let item = |key: &str, action: Box<dyn gpui::Action>| checked(key, action, false);
    use crate::tools::{
        ColumnLeft, ColumnRight, DeleteColumn, DeleteRow, InsertTable, RowAbove, RowBelow,
    };
    vec![
        Menu::new(text(cx, "menu.edit", &[])).items([
            item("office.find.open", Box::new(ely_office::FindReplace)),
            item("mark.copy.rich", Box::new(crate::export::CopyRich)),
        ]),
        Menu::new(text(cx, "mark.menu.table", &[])).items([
            item("mark.table.insert", Box::new(InsertTable)),
            MenuItem::separator(),
            item("mark.table.row_above", Box::new(RowAbove)),
            item("mark.table.row_below", Box::new(RowBelow)),
            item("mark.table.column_left", Box::new(ColumnLeft)),
            item("mark.table.column_right", Box::new(ColumnRight)),
            MenuItem::separator(),
            item("mark.table.delete_row", Box::new(DeleteRow)),
            item("mark.table.delete_column", Box::new(DeleteColumn)),
        ]),
        Menu::new(text(cx, "menu.view", &[])).items([
            MenuItem::action(label(Mode::Visual, cx), VisualMode),
            MenuItem::action(label(Mode::Source, cx), SourceMode),
            MenuItem::action(label(Mode::Preview, cx), PreviewMode),
            MenuItem::separator(),
            checked("mark.look.focus", Box::new(look::FocusMode), look.focus),
            checked(
                "mark.look.typewriter",
                Box::new(look::Typewriter),
                look.typewriter,
            ),
            MenuItem::separator(),
            checked(
                "mark.look.narrow",
                Box::new(look::Narrow),
                look.width == Width::Narrow,
            ),
            checked(
                "mark.look.medium",
                Box::new(look::Medium),
                look.width == Width::Medium,
            ),
            checked(
                "mark.look.wide",
                Box::new(look::Wide),
                look.width == Width::Wide,
            ),
            MenuItem::separator(),
            checked(
                "mark.look.sans",
                Box::new(look::Sans),
                look.font == Font::Sans,
            ),
            checked(
                "mark.look.serif",
                Box::new(look::Serif),
                look.font == Font::Serif,
            ),
            checked(
                "mark.look.mono",
                Box::new(look::Mono),
                look.font == Font::Mono,
            ),
            MenuItem::separator(),
            item("mark.look.larger", Box::new(look::Larger)),
            item("mark.look.smaller", Box::new(look::Smaller)),
            item("mark.look.actual", Box::new(look::ActualSize)),
            MenuItem::separator(),
            item("mark.outside.check", Box::new(CheckDisk)),
        ]),
    ]
}
