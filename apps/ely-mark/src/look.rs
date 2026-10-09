//! The writing view, kept across launches (MARK-09, MARK-12).

use std::{io, path::Path};

use gpui::{Context, actions};
use serde::{Deserialize, Serialize};

use crate::desk::Desk;

actions!(
    mark,
    [
        FocusMode, Typewriter, Narrow, Medium, Wide, Sans, Serif, Mono, Larger, Smaller, ActualSize
    ]
);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Width {
    Narrow,
    Medium,
    Wide,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Font {
    Sans,
    Serif,
    Mono,
}

/// How text shows; the Markdown never changes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Look {
    pub focus: bool,
    pub typewriter: bool,
    pub width: Width,
    pub font: Font,
    /// Percent of the app's text size.
    pub size: u32,
}

impl Default for Look {
    fn default() -> Self {
        Look {
            focus: false,
            typewriter: false,
            width: Width::Medium,
            font: Font::Sans,
            size: 100,
        }
    }
}

/// The kept look; none kept is the default.
pub fn load(file: &Path) -> io::Result<Look> {
    match std::fs::read(file) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Look::default()),
        Err(error) => Err(error),
    }
}

/// Sizes step by a tenth, from 70 to 200 percent.
const SIZES: (u32, u32, u32) = (70, 200, 10);

impl Desk {
    /// Changes the look; pages, file and menus follow.
    pub fn relook(&mut self, change: impl FnOnce(&mut Look), cx: &mut Context<Self>) {
        change(&mut self.look);
        self.look.size = self.look.size.clamp(SIZES.0, SIZES.1);
        log::info!("mark: the look is now {:?}", self.look);
        self.show_look(cx);
        let json = serde_json::to_vec_pretty(&self.look).expect("a look serializes");
        if let Err(error) = ely_files::save_atomic(&self.look_file, &json, 0) {
            log::error!("mark: the look did not save: {error}");
        }
        let menus = crate::keys::menus(&self.look, cx);
        ely_ui::set_document_menus(
            ely_platform::AppId::Mark,
            &ely_ui::commands::RENAMING,
            menus,
            cx,
        );
        cx.notify();
    }

    /// Every tab's page takes the look.
    pub fn show_look(&mut self, cx: &mut Context<Self>) {
        let look = serde_json::to_value(&self.look).expect("a look serializes");
        for tab in &self.core.tabs {
            let look = look.clone();
            tab.view.update(cx, |view, cx| view.set_look(look, cx));
        }
    }

    pub fn look_actions(element: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        use gpui::InteractiveElement as _;
        let step = SIZES.2;
        element
            .on_action(cx.listener(|desk, _: &FocusMode, _, cx| {
                desk.relook(|look| look.focus = !look.focus, cx)
            }))
            .on_action(cx.listener(|desk, _: &Typewriter, _, cx| {
                desk.relook(|look| look.typewriter = !look.typewriter, cx)
            }))
            .on_action(cx.listener(|desk, _: &Narrow, _, cx| {
                desk.relook(|look| look.width = Width::Narrow, cx)
            }))
            .on_action(cx.listener(|desk, _: &Medium, _, cx| {
                desk.relook(|look| look.width = Width::Medium, cx)
            }))
            .on_action(
                cx.listener(|desk, _: &Wide, _, cx| {
                    desk.relook(|look| look.width = Width::Wide, cx)
                }),
            )
            .on_action(
                cx.listener(|desk, _: &Sans, _, cx| desk.relook(|look| look.font = Font::Sans, cx)),
            )
            .on_action(
                cx.listener(|desk, _: &Serif, _, cx| {
                    desk.relook(|look| look.font = Font::Serif, cx)
                }),
            )
            .on_action(
                cx.listener(|desk, _: &Mono, _, cx| desk.relook(|look| look.font = Font::Mono, cx)),
            )
            .on_action(
                cx.listener(move |desk, _: &Larger, _, cx| {
                    desk.relook(|look| look.size += step, cx)
                }),
            )
            .on_action(cx.listener(move |desk, _: &Smaller, _, cx| {
                desk.relook(|look| look.size = look.size.saturating_sub(step), cx)
            }))
            .on_action(
                cx.listener(|desk, _: &ActualSize, _, cx| desk.relook(|look| look.size = 100, cx)),
            )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_kept_look_reads_back_and_none_is_default() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let file = dir.path().join("look.json");
        assert_eq!(
            super::load(&file).expect("none kept"),
            super::Look::default()
        );
        std::fs::write(
            &file,
            r#"{"focus":true,"typewriter":false,"width":"wide","font":"serif","size":120}"#,
        )
        .expect("a look");
        let look = super::load(&file).expect("it reads");
        assert!(look.focus);
        assert_eq!(
            (look.width, look.font, look.size),
            (super::Width::Wide, super::Font::Serif, 120)
        );
    }
}
