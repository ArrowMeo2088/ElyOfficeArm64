//! The wheel and Up, Down turn slides (SHOW-01).

use std::time::{Duration, Instant};

use gpui::{Context, EventEmitter, Keystroke, Modifiers, ScrollDelta, ScrollWheelEvent, TouchPhase};

use super::LokView;

/// Asks for the part `step` away from the shown one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turn(pub i32);

impl EventEmitter<Turn> for LokView {}

/// Trackpad pixels one turn takes.
const SWIPE: f32 = 50.0;
/// A pause this long ends a trackpad gesture.
const PAUSE: Duration = Duration::from_millis(250);

/// The wheel's progress towards a turn.
#[derive(Default)]
pub(super) struct Turning {
    lines: f32,
    pixels: f32,
    /// This gesture already turned.
    spent: bool,
    last: Option<Instant>,
}

impl Turning {
    /// The turn a wheel event makes; 0 for none.
    pub(super) fn wheel(&mut self, delta: ScrollDelta, phase: TouchPhase, now: Instant) -> i32 {
        let paused = self.last.is_none_or(|last| now.duration_since(last) >= PAUSE);
        self.last = Some(now);
        let ahead = match delta {
            // Each notch turns once.
            ScrollDelta::Lines(lines) => {
                if paused {
                    self.lines = 0.0;
                }
                self.lines += lines.y;
                let whole = self.lines.trunc();
                self.lines -= whole;
                whole
            }
            // One trackpad gesture turns once, momentum included.
            ScrollDelta::Pixels(pixels) => {
                if paused || phase == TouchPhase::Started {
                    (self.pixels, self.spent) = (0.0, false);
                }
                if self.spent {
                    return 0;
                }
                self.pixels += f32::from(pixels.y);
                if self.pixels.abs() < SWIPE {
                    return 0;
                }
                self.spent = true;
                self.pixels.signum()
            }
        };
        // Content moving down shows the part before.
        -(ahead as i32)
    }
}

impl LokView {
    /// The wheel and Up, Down turn parts from now on.
    pub fn turn_parts(&mut self) {
        self.turning.get_or_insert_default();
    }

    /// The part shown.
    pub fn part(&self) -> usize {
        self.part
    }

    /// Up and Down turn while nothing is picked or edited.
    pub(super) fn turn_key(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) -> bool {
        let idle = !self.caret() && self.graphic.is_none() && self.selection.is_empty();
        if self.turning.is_none() || !idle || keystroke.modifiers != Modifiers::none() {
            return false;
        }
        let step = match keystroke.key.as_str() {
            "down" => 1,
            "up" => -1,
            _ => return false,
        };
        cx.emit(Turn(step));
        true
    }

    /// The wheel turns while the whole page shows.
    pub(super) fn turn_wheel(&mut self, event: &ScrollWheelEvent, cx: &mut Context<Self>) -> bool {
        let (width, height) = self.extent();
        // A pixel of slack for rounding.
        let whole = self.geom.px_of(width) <= self.viewport.0 + 1.0
            && self.geom.px_of(height) <= self.viewport.1 + 1.0;
        let Some(turning) = self.turning.as_mut().filter(|_| whole) else {
            return false;
        };
        let step = turning.wheel(event.delta, event.touch_phase, Instant::now());
        if step != 0 {
            cx.emit(Turn(step));
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use gpui::{ScrollDelta, TouchPhase, point, px};

    use super::{PAUSE, Turning};

    #[test]
    fn each_notch_turns_once() {
        let (mut turning, now) = (Turning::default(), std::time::Instant::now());
        let notch = |y| ScrollDelta::Lines(point(0.0, y));
        assert_eq!(turning.wheel(notch(-1.0), TouchPhase::Moved, now), 1);
        assert_eq!(turning.wheel(notch(-1.0), TouchPhase::Moved, now), 1);
        assert_eq!(turning.wheel(notch(1.0), TouchPhase::Moved, now), -1);
        assert_eq!(turning.wheel(notch(0.5), TouchPhase::Moved, now), 0);
        assert_eq!(turning.wheel(notch(0.5), TouchPhase::Moved, now), -1);
    }

    #[test]
    fn a_swipe_turns_once_until_the_next() {
        let (mut turning, now) = (Turning::default(), std::time::Instant::now());
        let swipe = |y| ScrollDelta::Pixels(point(px(0.0), px(y)));
        assert_eq!(turning.wheel(swipe(-30.0), TouchPhase::Started, now), 0);
        assert_eq!(turning.wheel(swipe(-30.0), TouchPhase::Moved, now), 1);
        // Momentum after the turn turns nothing more.
        for _ in 0..20 {
            assert_eq!(turning.wheel(swipe(-80.0), TouchPhase::Moved, now), 0);
        }
        assert_eq!(turning.wheel(swipe(60.0), TouchPhase::Started, now), -1);
        // A pause ends a gesture without phases.
        let later = now + PAUSE;
        assert_eq!(turning.wheel(swipe(60.0), TouchPhase::Moved, later), -1);
    }
}
