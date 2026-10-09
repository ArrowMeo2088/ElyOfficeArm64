//! Zooming the page view about the pointer (LOK-04).

use gpui::{Context, EventEmitter, PinchEvent, Pixels, Point, ScrollDelta, ScrollWheelEvent, Window};

use super::LokView;

/// The zoom changed, by key, wheel or pinch.
pub struct Zoomed;

impl EventEmitter<Zoomed> for LokView {}

/// The steps of zooming in and out.
pub const ZOOM_STEPS: [f32; 10] = [0.25, 0.5, 0.67, 0.8, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];
const LEAST: f32 = 0.25;
const MOST: f32 = 4.0;

impl LokView {
    /// The zoom now; 1.0 is the page's actual size.
    pub fn zoom(&self) -> f32 {
        self.geom.zoom
    }

    /// Whether the zoom follows the viewport (a whole page).
    pub fn fitting(&self) -> bool {
        self.fit
    }

    /// Sets the zoom; the grid is repainted (LOK-04).
    pub fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        self.zoom_at(zoom, None, cx);
    }

    /// The next step in or out from the zoom now.
    pub fn zoom_step(&mut self, larger: bool, cx: &mut Context<Self>) {
        let now = self.geom.zoom;
        let next = match larger {
            true => ZOOM_STEPS.into_iter().find(|step| *step > now + 0.001).unwrap_or(MOST),
            false => ZOOM_STEPS.into_iter().rev().find(|step| *step < now - 0.001).unwrap_or(LEAST),
        };
        self.zoom_at(next, None, cx);
    }

    /// Zooms keeping `at` (window pixels) still; none keeps the middle.
    pub fn zoom_at(&mut self, zoom: f32, at: Option<Point<Pixels>>, cx: &mut Context<Self>) {
        let zoom = zoom.clamp(LEAST, MOST);
        let (ax, ay) = match at {
            Some(at) => (f32::from(at.x - self.origin.x), f32::from(at.y - self.origin.y)),
            None => (self.viewport.0 / 2.0, self.viewport.1 / 2.0),
        };
        let under = (self.scroll.0 + self.geom.twips_of(ax), self.scroll.1 + self.geom.twips_of(ay));
        self.fit = false;
        if (zoom - self.geom.zoom).abs() > 0.0001 {
            self.geom.zoom = zoom;
            self.retire_tiles();
            self.failed.clear();
        }
        self.scroll = (under.0 - self.geom.twips_of(ax), under.1 - self.geom.twips_of(ay));
        self.clamp_scroll();
        log::debug!("lok: zoom {:.0}%", zoom * 100.0);
        cx.emit(Zoomed);
        cx.notify();
    }

    /// Command with the wheel zooms; otherwise it scrolls.
    pub(super) fn on_wheel(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !event.modifiers.platform {
            return self.on_scroll(event, window, cx);
        }
        let dy = match event.delta {
            ScrollDelta::Pixels(point) => f32::from(point.y),
            ScrollDelta::Lines(point) => point.y * 40.0,
        };
        let zoom = self.geom.zoom * (dy / 200.0).exp();
        self.zoom_at(zoom, Some(event.position), cx);
    }

    /// A trackpad pinch zooms about its centre.
    pub(super) fn on_pinch(&mut self, event: &PinchEvent, _: &mut Window, cx: &mut Context<Self>) {
        let zoom = self.geom.zoom * (1.0 + event.delta);
        self.zoom_at(zoom, Some(event.position), cx);
    }
}
