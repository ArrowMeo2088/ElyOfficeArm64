//! AppKit's Command edit keys; off-screen Blink never gets them.

use cef::*;

const SHIFT: u32 = 1 << 1;
const CONTROL: u32 = 1 << 2;
const ALT: u32 = 1 << 3;
const COMMAND: u32 = 1 << 7;

/// What AppKit's standard bindings do, as page script.
pub(crate) fn script(windows_key: i32, modifiers: u32) -> Option<String> {
    if modifiers & COMMAND == 0 || modifiers & (CONTROL | ALT) != 0 {
        return None;
    }
    let alter = if modifiers & SHIFT != 0 {
        "extend"
    } else {
        "move"
    };
    let (direction, unit) = match windows_key {
        0x25 => ("backward", "lineboundary"),
        0x27 => ("forward", "lineboundary"),
        0x26 => ("backward", "documentboundary"),
        0x28 => ("forward", "documentboundary"),
        // Delete to the line's start or end.
        0x08 | 0x2E if modifiers & SHIFT == 0 => {
            let (direction, command) = match windows_key {
                0x08 => ("backward", "delete"),
                _ => ("forward", "forwardDelete"),
            };
            return Some(format!(
                "{{const s=getSelection();if(s.isCollapsed)s.modify('extend','{direction}','lineboundary');document.execCommand('{command}')}}"
            ));
        }
        _ => return None,
    };
    Some(format!(
        "{{const s=getSelection();s.modify('{alter}','{direction}','{unit}');s.focusNode?.parentElement?.scrollIntoView({{block:'nearest'}})}}"
    ))
}

wrap_keyboard_handler! {
    pub(crate) struct Keys;

    impl KeyboardHandler {
        fn on_key_event(
            &self,
            browser: Option<&mut Browser>,
            event: Option<&KeyEvent>,
            _os_event: *mut u8,
        ) -> ::std::os::raw::c_int {
            let (Some(browser), Some(event)) = (browser, event) else { return 0 };
            let down = matches!(event.type_, KeyEventType::RAWKEYDOWN | KeyEventType::KEYDOWN);
            if !down || event.focus_on_editable_field == 0 {
                return 0;
            }
            let Some(script) = script(event.windows_key_code, event.modifiers) else { return 0 };
            let Some(frame) = browser.focused_frame() else { return 0 };
            log::debug!("web: AppKit edit key {}", event.windows_key_code);
            frame.execute_java_script(Some(&CefString::from(script.as_str())), None, 0);
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_keys_only() {
        assert!(
            script(0x28, COMMAND)
                .unwrap()
                .contains("'move','forward','documentboundary'")
        );
        assert!(
            script(0x25, COMMAND | SHIFT)
                .unwrap()
                .contains("'extend','backward','lineboundary'")
        );
        assert!(
            script(0x08, COMMAND)
                .unwrap()
                .contains("execCommand('delete')")
        );
        assert!(script(0x2E, COMMAND).unwrap().contains("forwardDelete"));
        assert_eq!(script(0x28, 0), None);
        assert_eq!(script(0x28, COMMAND | ALT), None);
        assert_eq!(script(0x41, COMMAND), None);
    }
}
