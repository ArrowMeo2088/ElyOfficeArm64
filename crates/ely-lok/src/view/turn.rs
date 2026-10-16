//! The wheel and Up, Down turn slides (SHOW-01).

use std::time::{Duration, Instant};

use gpui::{Context, EventEmitter, Keystroke, Modifiers, ScrollDelta, ScrollWheelEvent, TouchPhase};

use super::LokView;
use super::input::{Input, vcl_key};

/// Asks for the part `step` away from the shown one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turn(pub i32);

impl EventEmitter<Turn> for LokView {}

/// Trackpad pixels one turn takes.
const SWIPE: f32 = 50.0;
/// A pause this long ends a gesture without phases.
const PAUSE: Duration = Duration::from_millis(250);

/// Lines one wheel notch scrolls on this platform.
pub fn notch() -> f32 {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETWHEELSCROLLLINES, SystemParametersInfoW};
        let mut lines = 3u32;
        // Same setting gpui scales the wheel by.
        unsafe { SystemParametersInfoW(SPI_GETWHEELSCROLLLINES, 0, (&raw mut lines).cast(), 0) };
        lines as f32
    }
    #[cfg(target_os = "macos")]
    {
        1.0
    }
    // gpui's X11 and Wayland scroll three lines a notch.
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        3.0
    }
}

/// The wheel's progress towards a turn.
#[derive(Default)]
pub(super) struct Turning {
    /// This gesture's travel, in turns.
    travel: f32,
    /// This gesture already turned.
    spent: bool,
    /// The platform began a gesture it has not ended.
    held: bool,
    /// Fractional lines: a smooth touchpad, not a wheel.
    smooth: bool,
    last: Option<Instant>,
}

impl Turning {
    /// The turn a wheel event makes; 0 for none.
    pub(super) fn wheel(&mut self, delta: ScrollDelta, phase: TouchPhase, now: Instant, notch: f32) -> i32 {
        let paused = self.last.is_none_or(|last| now.duration_since(last) >= PAUSE);
        self.last = Some(now);
        let travel = match delta {
            ScrollDelta::Lines(_) if notch <= 0.0 => return 0,
            ScrollDelta::Lines(lines) => {
                let notches = lines.y / notch;
                let whole = notches.round();
                // Whole notches outside a smooth stream: each turns once.
                if whole != 0.0 && (notches - whole).abs() < 1e-3 && (paused || !self.smooth) {
                    (self.travel, self.spent, self.smooth) = (0.0, false, false);
                    return -(whole as i32);
                }
                if paused || !self.smooth {
                    (self.travel, self.spent) = (0.0, false);
                }
                self.smooth = true;
                notches
            }
            ScrollDelta::Pixels(pixels) => {
                self.smooth = false;
                // Without phases, only a pause ends a gesture.
                if phase == TouchPhase::Started || paused && !self.held {
                    (self.travel, self.spent) = (0.0, false);
                }
                self.held = match phase {
                    TouchPhase::Started => true,
                    TouchPhase::Ended | TouchPhase::Cancelled => false,
                    TouchPhase::Moved => self.held,
                };
                f32::from(pixels.y) / SWIPE
            }
        };
        // One gesture turns once, momentum included.
        if self.spent {
            return 0;
        }
        self.travel += travel;
        if self.travel.abs() < 1.0 {
            return 0;
        }
        self.spent = true;
        // Content moving down shows the part before.
        -(self.travel.signum() as i32)
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

    /// Queues Up, Down; the worker turns if nothing is picked.
    pub(super) fn turn_key(&mut self, keystroke: &Keystroke) -> bool {
        if self.turning.is_none() || keystroke.modifiers != Modifiers::none() {
            return false;
        }
        let step = match keystroke.key.as_str() {
            "down" => 1,
            "up" => -1,
            _ => return false,
        };
        let Some((char_code, key_code)) = vcl_key(keystroke) else {
            return false;
        };
        self.send(Input::Turn { char_code, key_code, step });
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
        let step = turning.wheel(event.delta, event.touch_phase, Instant::now(), notch());
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

    const NOTCH: f32 = 3.0;

    fn lines(y: f32) -> ScrollDelta {
        ScrollDelta::Lines(point(0.0, y))
    }

    fn pixels(y: f32) -> ScrollDelta {
        ScrollDelta::Pixels(point(px(0.0), px(y)))
    }

    #[test]
    fn each_notch_turns_once() {
        let (mut turning, now) = (Turning::default(), std::time::Instant::now());
        let mut wheel = |y| turning.wheel(lines(y), TouchPhase::Moved, now, NOTCH);
        assert_eq!(wheel(-3.0), 1, "Windows and Linux scroll three lines a notch");
        assert_eq!(wheel(-3.0), 1);
        assert_eq!(wheel(3.0), -1);
        assert_eq!(wheel(-6.0), 2, "two notches in one event");
        let mut mac = Turning::default();
        assert_eq!(mac.wheel(lines(-1.0), TouchPhase::Moved, now, 1.0), 1);
    }

    #[test]
    fn smooth_lines_turn_once_a_gesture() {
        let (mut turning, now) = (Turning::default(), std::time::Instant::now());
        // An X11 or Windows touchpad: fractions, no phases.
        let turned: i32 = (0..30).map(|_| turning.wheel(lines(-0.4), TouchPhase::Moved, now, NOTCH)).sum();
        assert_eq!(turned, 1, "one swipe turns once");
        assert_eq!(turning.wheel(lines(-3.0), TouchPhase::Moved, now, NOTCH), 0, "a whole step mid-swipe");
        let later = now + PAUSE;
        assert_eq!(turning.wheel(lines(-3.0), TouchPhase::Moved, later, NOTCH), 1, "a pause ends the swipe");
    }

    #[test]
    fn a_swipe_turns_once_until_the_next() {
        let (mut turning, now) = (Turning::default(), std::time::Instant::now());
        let mut swipe = |y, phase, at| turning.wheel(pixels(y), phase, at, NOTCH);
        assert_eq!(swipe(-30.0, TouchPhase::Started, now), 0);
        assert_eq!(swipe(-30.0, TouchPhase::Moved, now), 1);
        // A pause with fingers down is the same gesture.
        assert_eq!(swipe(-60.0, TouchPhase::Moved, now + PAUSE * 2), 0);
        assert_eq!(swipe(0.0, TouchPhase::Ended, now + PAUSE * 2), 0);
        // Momentum after the turn turns nothing more.
        for _ in 0..20 {
            assert_eq!(swipe(-80.0, TouchPhase::Moved, now + PAUSE * 2), 0);
        }
        assert_eq!(swipe(60.0, TouchPhase::Started, now + PAUSE * 2), -1);
        assert_eq!(swipe(0.0, TouchPhase::Ended, now + PAUSE * 2), 0);
        // Without phases a pause ends a gesture (Wayland).
        let later = now + PAUSE * 4;
        assert_eq!(swipe(60.0, TouchPhase::Moved, later), -1);
        assert_eq!(swipe(60.0, TouchPhase::Moved, later), 0);
        assert_eq!(swipe(60.0, TouchPhase::Moved, later + PAUSE), -1);
    }
}
