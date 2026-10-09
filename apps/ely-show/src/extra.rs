//! What ElyShow keeps per deck.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use ely_lok::{
    Sheets,
    history::Step,
    show::{Design, Objects},
};
use gpui::RenderImage;

/// Parts, edits, caret and selected frame.
pub type Seen = (u64, u64, bool, Option<ely_lok::Rect>);

/// What ElyShow keeps per deck.
#[derive(Default)]
pub struct Extra {
    pub slides: Sheets,
    /// The document as the last read saw it.
    pub read_at: Option<ely_lok::Doc>,
    /// What the view showed at the last read.
    pub read: Option<Seen>,
    pub reading: bool,
    pub design: Option<Design>,
    pub objects: Option<Objects>,
    pub notes: Option<ely_lok::show::Notes>,
    pub remarks: Vec<ely_lok::show::Remark>,
    /// The shown slide's transition and effects; none on a master.
    pub motion: Option<(ely_lok::show::Transition, Vec<ely_lok::show::Effect>)>,
    pub media: Vec<ely_lok::show::Media>,
    /// Counts completed reads.
    pub reads: u64,
    /// The slide size in twips.
    pub size: (i64, i64),
    /// Undo and redo lists at the last read.
    pub history: Option<(Vec<Step>, Vec<Step>)>,
    /// A size sent, in 1/100 mm, until read back.
    pub size_asked: Option<(i64, i64)>,
    /// A transition sent for a slide, until read back.
    pub transition_asked: Option<(String, ely_lok::show::Transition, u64)>,
    /// Pictures by slide name; stale ones show until repainted.
    pub thumbs: HashMap<String, Arc<RenderImage>>,
    pub retired: ely_gpui_component::primitives::Retired,
    pub stale: HashSet<String>,
    /// Pictures that failed; retried after the next read.
    pub failed: HashSet<String>,
    pub painting: bool,
    /// The parts are masters.
    pub masters: bool,
    /// The scheme the pictures were painted with.
    pub dark: Option<bool>,
    /// The view was fitted to the slide once.
    pub fitted: bool,
}

impl Extra {
    /// The size to show, in 1/100 mm: asked, else read.
    pub fn shown_size(&self) -> (i64, i64) {
        self.size_asked
            .unwrap_or_else(|| crate::design::mm100(self.size))
    }
}
