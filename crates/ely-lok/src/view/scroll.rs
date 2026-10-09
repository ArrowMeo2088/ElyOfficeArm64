//! Scrolling the page view (LOK-04).

use gpui::{
    Bounds, Context, Pixels, Point, ScrollDelta, ScrollWheelEvent, Window, point, px, size,
};

use super::{LokView, input::Input};
use crate::{Mouse, Rect};

impl LokView {
    /// Scrolls by view pixels, clamped to the document (LOK-04).
    pub(super) fn scroll_by(&mut self, dx: f32, dy: f32, cx: &mut Context<Self>) {
        self.scroll.0 -= self.geom.twips_of(dx);
        self.scroll.1 -= self.geom.twips_of(dy);
        self.clamp_scroll();
        cx.notify();
    }

    /// Keeps the viewport inside the document (LOK-04).
    pub(super) fn clamp_scroll(&mut self) {
        let (vw, vh) = (
            self.geom.twips_of(self.viewport.0),
            self.geom.twips_of(self.viewport.1),
        );
        let extent = self.extent();
        self.scroll.0 = self.scroll.0.clamp(0, (extent.0 - vw).max(0));
        self.scroll.1 = self.scroll.1.clamp(0, (extent.1 - vh).max(0));
    }

    /// Scrolls the wheel in twips, clamped to the document (LOK-04).
    pub(super) fn on_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (dx, dy) = match event.delta {
            ScrollDelta::Pixels(point) => (f32::from(point.x), f32::from(point.y)),
            ScrollDelta::Lines(point) => (point.x * 40.0, point.y * 40.0),
        };
        self.scroll_by(dx, dy, cx);
    }

    /// Scrolls `rect` into view; frozen parts always show.
    pub fn show(&mut self, (x, y, w, h): Rect, cx: &mut Context<Self>) {
        let (fx, fy) = self.frozen;
        let (vw, vh) = (
            self.geom.twips_of(self.viewport.0) - fx,
            self.geom.twips_of(self.viewport.1) - fy,
        );
        // Room past the target, so the clamp lets it show.
        self.reach = (
            self.reach.0.max(x + w + vw + fx),
            self.reach.1.max(y + h + vh + fy),
        );
        if x >= fx && (x < fx + self.scroll.0 || x + w > fx + self.scroll.0 + vw) {
            self.scroll.0 = x - fx - vw / 2;
        }
        if y >= fy && (y < fy + self.scroll.1 || y + h > fy + self.scroll.1 + vh) {
            self.scroll.1 = y - fy - vh / 3;
        }
        self.clamp_scroll();
        cx.notify();
    }

    /// Freezes what lies before `frozen`, in twips.
    pub fn set_frozen(&mut self, frozen: (i32, i32), cx: &mut Context<Self>) {
        if self.frozen != frozen {
            log::info!("lok: frozen at {frozen:?}");
            self.frozen = frozen;
            cx.notify();
        }
    }

    /// Scrolls `rect` into view and puts the cursor there.
    pub fn reveal(&mut self, rect: Rect, cx: &mut Context<Self>) {
        self.show(rect, cx);
        let (x, y, _, h) = rect;
        log::info!("lok: reveal at {x},{y}");
        for kind in [0, 1] {
            let (count, buttons, modifiers) = (1, 1, 0);
            let y = y + h / 2;
            self.send(Input::Mouse(Mouse {
                kind,
                x,
                y,
                count,
                buttons,
                modifiers,
            }));
        }
    }

    /// The scroll offset in twips.
    pub fn scroll(&self) -> (i32, i32) {
        self.scroll
    }

    /// The document extent the view may scroll over.
    pub(super) fn extent(&self) -> (i32, i32) {
        // A fitted page is the page, whatever the cursor reached.
        if self.fit {
            return self.doc_size;
        }
        (
            self.doc_size.0.max(self.reach.0),
            self.doc_size.1.max(self.reach.1),
        )
    }

    /// The document point under a view position (LOK-04).
    pub fn doc_point(&self, position: Point<Pixels>) -> (i32, i32) {
        let local = position - self.origin;
        let (ax, ay) = self.axes();
        (
            ax.doc(self.geom.twips_of(f32::from(local.x) - self.spare.0)),
            ay.doc(self.geom.twips_of(f32::from(local.y) - self.spare.1)),
        )
    }

    /// The panes of each axis.
    pub(super) fn axes(&self) -> (super::panes::Axis, super::panes::Axis) {
        let axis = |frozen, scroll| super::panes::Axis { frozen, scroll };
        (
            axis(self.frozen.0, self.scroll.0),
            axis(self.frozen.1, self.scroll.1),
        )
    }

    /// The window point of a document point in twips (LOK-04).
    pub fn window_point(&self, (x, y): (i32, i32)) -> gpui::Point<Pixels> {
        let (ax, ay) = self.axes();
        let (x, y) = (self.geom.px_of(ax.show(x)), self.geom.px_of(ay.show(y)));
        self.origin + point(px(self.spare.0 + x), px(self.spare.1 + y))
    }

    /// A document rect in window pixels, panes applied.
    pub(super) fn screen_rect(&self, (x, y, w, h): crate::Rect) -> Bounds<Pixels> {
        Bounds::from_corners(self.window_point((x, y)), self.window_point((x + w, y + h)))
    }

    /// The cursor rect in window pixels.
    pub fn cursor_bounds(&self) -> Option<Bounds<Pixels>> {
        let (x, y, w, h) = self.cursor?;
        Some(Bounds {
            origin: self.window_point((x, y)),
            size: size(px(self.geom.px_of(w.max(1))), px(self.geom.px_of(h))),
        })
    }
}
