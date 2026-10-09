//! GPUI's mouse, keys, focus and IME, as CEF events.

use std::ops::Range;

use cef::*;
use gpui::{
    App, Bounds, Context, Div, Entity, EntityInputHandler, InteractiveElement as _, KeyDownEvent,
    KeyUpEvent, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, ScrollDelta, ScrollWheelEvent, UTF16Selection, Window, point, px, size,
};

use crate::{keys, view::WebView};

gpui::actions!(web, [PageTab, PageBackTab]);

/// The page's key context: Tab stays in the page.
const CONTEXT: &str = "WebView";

/// Tab and Shift-Tab go to the page, not to focus.
pub(crate) fn bind(cx: &mut App) {
    cx.bind_keys([
        gpui::KeyBinding::new("tab", PageTab, Some(CONTEXT)),
        gpui::KeyBinding::new("shift-tab", PageBackTab, Some(CONTEXT)),
    ]);
}

/// CEF's `cef_event_flags_t` for these modifiers.
pub(crate) fn flags(modifiers: &Modifiers) -> u32 {
    let mut flags = 0;
    for (on, bit) in [
        (modifiers.shift, 1 << 1),
        (modifiers.control, 1 << 2),
        (modifiers.alt, 1 << 3),
        (modifiers.platform, 1 << 7),
    ] {
        if on {
            flags |= bit;
        }
    }
    flags
}

fn button(button: MouseButton) -> Option<(MouseButtonType, u32)> {
    match button {
        MouseButton::Left => Some((MouseButtonType::LEFT, 1 << 4)),
        MouseButton::Middle => Some((MouseButtonType::MIDDLE, 1 << 5)),
        MouseButton::Right => Some((MouseButtonType::RIGHT, 1 << 6)),
        _ => None,
    }
}

impl WebView {
    /// A window point as a page point.
    fn at(&self, position: Point<Pixels>, modifiers: u32) -> MouseEvent {
        let local = position - self.bounds.origin;
        MouseEvent {
            x: f32::from(local.x).round() as i32,
            y: f32::from(local.y).round() as i32,
            modifiers,
        }
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        let Some((kind, held)) = button(event.button) else {
            return;
        };
        let at = self.at(event.position, flags(&event.modifiers) | held);
        let count = event.click_count as i32;
        self.on_host(move |host| host.send_mouse_click_event(Some(&at), kind, false as _, count));
    }

    fn mouse_up(&mut self, event: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        let Some((kind, _)) = button(event.button) else {
            return;
        };
        let at = self.at(event.position, flags(&event.modifiers));
        let count = event.click_count as i32;
        self.on_host(move |host| host.send_mouse_click_event(Some(&at), kind, true as _, count));
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, _: &mut Context<Self>) {
        let held = event
            .pressed_button
            .and_then(button)
            .map_or(0, |(_, bit)| bit);
        let at = self.at(event.position, flags(&event.modifiers) | held);
        self.on_host(move |host| host.send_mouse_move_event(Some(&at), false as _));
    }

    fn scroll(&mut self, event: &ScrollWheelEvent, _: &mut Window, _: &mut Context<Self>) {
        // Lines as Chromium counts them: forty points.
        let delta = match event.delta {
            ScrollDelta::Pixels(delta) => (f32::from(delta.x), f32::from(delta.y)),
            ScrollDelta::Lines(delta) => (delta.x * 40.0, delta.y * 40.0),
        };
        let at = self.at(event.position, flags(&event.modifiers));
        let (x, y) = (delta.0.round() as i32, delta.1.round() as i32);
        self.on_host(move |host| host.send_mouse_wheel_event(Some(&at), x, y));
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let stroke = &event.keystroke;
        // Edit commands Chromium only takes from its own menu.
        let edit = stroke.key.clone();
        if keys::edit_chord(&stroke.modifiers) && matches!(edit.as_str(), "c" | "x" | "v" | "a") {
            // The owner may take a paste, such as an image.
            if edit == "v" && self.paste.clone().is_some_and(|take| take(window, cx)) {
                return cx.stop_propagation();
            }
            self.on_browser(move |browser| {
                let Some(frame) = browser.main_frame() else {
                    return;
                };
                match edit.as_str() {
                    "c" => frame.copy(),
                    "x" => frame.cut(),
                    "v" => frame.paste(),
                    _ => frame.select_all(),
                }
            });
            return cx.stop_propagation();
        }
        let Some(code) = keys::code(&stroke.key) else {
            return log::debug!("web: no key code for {}", stroke.key);
        };
        let event = KeyEvent {
            modifiers: flags(&stroke.modifiers),
            windows_key_code: code.windows,
            native_key_code: code.native,
            ..Default::default()
        };
        let chord = stroke.modifiers.control || stroke.modifiers.platform;
        match (keys::character(&stroke.key), &stroke.key_char) {
            // Typed text arrives by the input handler, which sends it.
            (None, Some(_)) if !chord => self.typing = Some(event),
            (character, typed) => {
                let character = character
                    .or_else(|| typed.as_deref().and_then(|t| t.encode_utf16().next()))
                    .unwrap_or(0);
                self.press(event, character);
            }
        }
    }

    /// Tab, with Shift when `back`, down and up.
    fn tab(&mut self, back: bool) {
        let code = keys::code("tab").expect("tab has a key code");
        let event = KeyEvent {
            modifiers: if back { 1 << 1 } else { 0 },
            windows_key_code: code.windows,
            native_key_code: code.native,
            ..Default::default()
        };
        self.press(event.clone(), 0x09);
        let up = KeyEvent {
            type_: KeyEventType::KEYUP,
            character: 0x09,
            unmodified_character: 0x09,
            ..event
        };
        self.on_host(move |host| host.send_key_event(Some(&up)));
    }

    /// A key down and its character, as cefclient sends them.
    fn press(&self, event: KeyEvent, character: u16) {
        let mut event = KeyEvent {
            type_: KeyEventType::KEYDOWN,
            character,
            unmodified_character: character,
            ..event
        };
        self.on_host(move |host| {
            host.send_key_event(Some(&event));
            event.type_ = KeyEventType::CHAR;
            host.send_key_event(Some(&event));
        });
    }

    fn key_up(&mut self, event: &KeyUpEvent, _: &mut Window, _: &mut Context<Self>) {
        let stroke = &event.keystroke;
        let Some(code) = keys::code(&stroke.key) else {
            return;
        };
        // An up without its character reads as another down.
        let character = keys::character(&stroke.key)
            .or_else(|| {
                stroke
                    .key_char
                    .as_deref()
                    .and_then(|t| t.encode_utf16().next())
            })
            .or_else(|| stroke.key.encode_utf16().next())
            .unwrap_or(0);
        let up = KeyEvent {
            type_: KeyEventType::KEYUP,
            modifiers: flags(&stroke.modifiers),
            windows_key_code: code.windows,
            native_key_code: code.native,
            character,
            unmodified_character: character,
            ..Default::default()
        };
        self.on_host(move |host| host.send_key_event(Some(&up)));
    }
}

/// The view's listeners on its outer element.
pub(crate) fn listen(element: Div, cx: &mut Context<WebView>) -> Div {
    element
        .key_context(CONTEXT)
        .on_action(cx.listener(|view, _: &PageTab, _, _| view.tab(false)))
        .on_action(cx.listener(|view, _: &PageBackTab, _, _| view.tab(true)))
        .on_mouse_down(MouseButton::Left, cx.listener(WebView::mouse_down))
        .on_mouse_down(MouseButton::Right, cx.listener(WebView::mouse_down))
        .on_mouse_down(MouseButton::Middle, cx.listener(WebView::mouse_down))
        .on_mouse_up(MouseButton::Left, cx.listener(WebView::mouse_up))
        .on_mouse_up(MouseButton::Right, cx.listener(WebView::mouse_up))
        .on_mouse_up(MouseButton::Middle, cx.listener(WebView::mouse_up))
        .on_mouse_move(cx.listener(WebView::mouse_move))
        .on_scroll_wheel(cx.listener(WebView::scroll))
        .on_key_down(cx.listener(WebView::key_down))
        .on_key_up(cx.listener(WebView::key_up))
}

/// CEF's page focus follows the view's.
pub(crate) fn follow_focus(view: &Entity<WebView>, window: &mut Window, cx: &mut App) {
    view.update(cx, |this, cx| {
        let focus = this.focus.clone();
        cx.on_focus(&focus, window, |view, _, _| {
            view.on_host(|host| host.set_focus(true as _));
        })
        .detach();
        cx.on_blur(&focus, window, |view, _, _| {
            view.on_host(|host| host.set_focus(false as _));
        })
        .detach();
    });
}

/// CEF's "no range": both ends at the maximum.
const NO_RANGE: cef::Range = cef::Range {
    from: u32::MAX,
    to: u32::MAX,
};

fn utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

impl EntityInputHandler for WebView {
    /// The IME sees none of the page's text.
    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        None
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.composing.clone()
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        if self.composing.take().is_some() {
            self.on_host(|host| host.ime_finish_composing_text(false as _));
        }
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        let typed = self.typing.take();
        let was = self.composing.take();
        let units = utf16(text);
        // One character, no composition: a key press, as cefclient.
        if let (Some(event), None, [character]) = (typed, was, units.as_slice()) {
            return self.press(event, *character);
        }
        let text = CefString::from(text);
        self.on_host(move |host| host.ime_commit_text(Some(&text), Some(&NO_RANGE), 0));
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        let length = utf16(new_text).len();
        self.composing = (length > 0).then_some(0..length);
        let selection = new_selected_range.unwrap_or(length..length);
        let range = cef::Range {
            from: selection.start as _,
            to: selection.end as _,
        };
        let text = CefString::from(new_text);
        self.on_host(move |host| {
            host.ime_set_composition(Some(&text), None, Some(&NO_RANGE), Some(&range))
        });
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        element: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let surface = self.surface.lock().expect("the surface lock");
        let (x, y, w, h) = *surface.ime.get(range.start).or(surface.ime.last())?;
        Some(Bounds::new(
            element.origin + point(px(x), px(y)),
            size(px(w), px(h)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
