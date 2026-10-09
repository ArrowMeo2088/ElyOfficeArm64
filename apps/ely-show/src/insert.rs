//! The Insert and View menus (SHOW-03 to 06).

use ely_office::prelude::*;
use gpui::{App, Context, Div, InteractiveElement as _, Menu, MenuItem, actions};

use crate::deck::Deck;

actions!(
    show,
    [
        InsertTextBox,
        InsertRectangle,
        InsertEllipse,
        InsertTriangle,
        InsertLine,
        InsertArrow,
        InsertConnector,
        InsertPicture,
        InsertMedia,
        InsertTable,
        InsertChart,
        InsertRemark,
        ShowRemarks,
        SnapToGrid,
        SnapToGuides,
        AddHorizontalGuide,
        AddVerticalGuide
    ]
);

/// Insert and View; View checks what is on.
pub fn menus((grid, guides, remarks): (bool, bool, bool), cx: &App) -> [Menu; 2] {
    let label = |key: &str| Deck::text(cx, key, &[]);
    let insert = Menu::new(label("menu.insert")).items([
        MenuItem::action(label("show.insert.text"), InsertTextBox),
        MenuItem::separator(),
        MenuItem::action(label("show.insert.rectangle"), InsertRectangle),
        MenuItem::action(label("show.insert.ellipse"), InsertEllipse),
        MenuItem::action(label("show.insert.triangle"), InsertTriangle),
        MenuItem::action(label("show.insert.line"), InsertLine),
        MenuItem::action(label("show.insert.arrow"), InsertArrow),
        MenuItem::action(label("show.insert.connector"), InsertConnector),
        MenuItem::separator(),
        MenuItem::action(label("show.insert.picture"), InsertPicture),
        MenuItem::action(label("show.insert.media"), InsertMedia),
        MenuItem::action(label("show.insert.table"), InsertTable),
        MenuItem::action(label("show.insert.chart"), InsertChart),
        MenuItem::separator(),
        MenuItem::action(label("show.insert.remark"), InsertRemark),
    ]);
    let view = Menu::new(label("menu.view")).items([
        MenuItem::action(label("show.view.remarks"), ShowRemarks).checked(remarks),
        MenuItem::separator(),
        MenuItem::action(label("show.view.snap_grid"), SnapToGrid).checked(grid),
        MenuItem::action(label("show.view.snap_guides"), SnapToGuides).checked(guides),
        MenuItem::separator(),
        MenuItem::action(label("show.view.guide_across"), AddHorizontalGuide),
        MenuItem::action(label("show.view.guide_down"), AddVerticalGuide),
        MenuItem::separator(),
    ]);
    let view = view.items(ely_office::zoom_items(true, cx));
    [insert, view]
}

/// Binds an action to an object command.
macro_rules! run {
    ($root:expr, $cx:expr, $action:ty, $id:literal, $values:expr) => {
        $root.on_action(
            $cx.listener(|deck, _: &$action, _, cx| deck.object_command($id, $values, cx)),
        )
    };
}

pub fn bind(root: Div, cx: &mut Context<Deck>) -> Div {
    let root = run!(root, cx, InsertTextBox, "draw.text", &["true"]);
    let root = run!(
        root,
        cx,
        InsertRectangle,
        "draw.shape",
        &["rectangle", "true"]
    );
    let root = run!(root, cx, InsertEllipse, "draw.shape", &["ellipse", "true"]);
    let root = run!(
        root,
        cx,
        InsertTriangle,
        "draw.shape",
        &["isosceles-triangle", "true"]
    );
    let root = run!(root, cx, InsertLine, "draw.line", &[]);
    let root = run!(root, cx, InsertArrow, "draw.arrow", &[]);
    let root = run!(root, cx, InsertConnector, "draw.connector", &[]);
    let root = run!(root, cx, SnapToGrid, "view.snap_grid", &[]);
    let root = run!(root, cx, SnapToGuides, "view.snap_guides", &[]);
    root.on_action(cx.listener(|deck, _: &InsertPicture, _, cx| deck.pick_picture(false, cx)))
        .on_action(cx.listener(|deck, _: &InsertMedia, _, cx| deck.pick_media(cx)))
        .on_action(cx.listener(|deck, _: &InsertTable, _, cx| deck.ask_table(cx)))
        .on_action(cx.listener(|deck, _: &InsertChart, _, cx| deck.ask_chart(cx)))
        .on_action(
            cx.listener(|deck, _: &InsertRemark, window, cx| deck.ask_remark(None, window, cx)),
        )
        .on_action(cx.listener(|deck, _: &ShowRemarks, _, cx| deck.toggle_remarks(cx)))
        .on_action(cx.listener(|deck, _: &AddHorizontalGuide, _, cx| deck.add_guide(false, cx)))
        .on_action(cx.listener(|deck, _: &AddVerticalGuide, _, cx| deck.add_guide(true, cx)))
}

impl Deck {
    /// Runs an object command; the next read shows the result.
    pub fn object_command(&mut self, id: &str, values: &[&str], cx: &mut Context<Self>) {
        let Some(tab) = self.core.active else {
            return;
        };
        self.run_in(Some(tab), id, values, cx);
        self.force_reread(tab, cx);
    }

    /// Inserts a picture, or replaces the selected one.
    pub fn pick_picture(&mut self, replace: bool, cx: &mut Context<Self>) {
        self.pick(false, cx, move |deck, path, cx| {
            let url = ely_lok::file_url(&path);
            let id = if replace {
                "image.replace"
            } else {
                "image.insert"
            };
            deck.object_command(id, &[&url], cx)
        });
    }

    /// Shows comments, or properties, in the right column.
    fn toggle_remarks(&mut self, cx: &mut Context<Self>) {
        self.remarks_open = !self.remarks_open;
        self.snap_menus = None;
        self.sync_menus(cx);
        cx.notify();
    }

    /// A guide through the middle of the slide.
    fn add_guide(&mut self, vertical: bool, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else {
            return;
        };
        let (w, h) = tab.extra.shown_size();
        let (kind, x, y) = if vertical {
            ("1", w / 2, 0)
        } else {
            ("0", 0, h / 2)
        };
        self.object_command("guide.add", &[kind, &x.to_string(), &y.to_string()], cx);
    }
}
