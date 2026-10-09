//! GPUI key names as the codes Chromium reads.

use gpui::Modifiers;

/// Windows' virtual key, and macOS's hardware key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Code {
    pub windows: i32,
    pub native: i32,
}

/// Letters, then digits, as macOS numbers its keys.
const MAC_LETTERS: [i32; 26] = [
    0, 11, 8, 2, 14, 3, 5, 4, 34, 38, 40, 37, 46, 45, 31, 35, 12, 15, 1, 17, 32, 9, 13, 7, 16, 6,
];
const MAC_DIGITS: [i32; 10] = [29, 18, 19, 20, 21, 23, 22, 26, 28, 25];
const MAC_F: [i32; 12] = [122, 120, 99, 118, 96, 97, 98, 100, 101, 109, 103, 111];

/// Named keys: GPUI's name, Windows, macOS.
const NAMED: &[(&str, i32, i32)] = &[
    ("backspace", 0x08, 51),
    ("tab", 0x09, 48),
    ("enter", 0x0D, 36),
    ("escape", 0x1B, 53),
    ("space", 0x20, 49),
    ("pageup", 0x21, 116),
    ("pagedown", 0x22, 121),
    ("end", 0x23, 119),
    ("home", 0x24, 115),
    ("left", 0x25, 123),
    ("up", 0x26, 126),
    ("right", 0x27, 124),
    ("down", 0x28, 125),
    ("delete", 0x2E, 117),
    (";", 0xBA, 41),
    ("=", 0xBB, 24),
    (",", 0xBC, 43),
    ("-", 0xBD, 27),
    (".", 0xBE, 47),
    ("/", 0xBF, 44),
    ("`", 0xC0, 50),
    ("[", 0xDB, 33),
    ("\\", 0xDC, 42),
    ("]", 0xDD, 30),
    ("'", 0xDE, 39),
];

pub fn code(key: &str) -> Option<Code> {
    if let Some(&(_, windows, native)) = NAMED.iter().find(|(name, _, _)| *name == key) {
        return Some(Code { windows, native });
    }
    let mut chars = key.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        let n: usize = key.strip_prefix('f')?.parse().ok()?;
        let native = *MAC_F.get(n.checked_sub(1)?)?;
        return Some(Code {
            windows: 0x6F + n as i32,
            native,
        });
    };
    match c {
        'a'..='z' => Some(Code {
            windows: c.to_ascii_uppercase() as i32,
            native: MAC_LETTERS[(c as u8 - b'a') as usize],
        }),
        '0'..='9' => Some(Code {
            windows: c as i32,
            native: MAC_DIGITS[(c as u8 - b'0') as usize],
        }),
        _ => None,
    }
}

/// The character AppKit gives keys that type none.
#[cfg(target_os = "macos")]
pub fn character(key: &str) -> Option<u16> {
    let n: Option<u16> = key.strip_prefix('f').and_then(|n| n.parse().ok());
    Some(match key {
        "backspace" => 0x7F,
        "tab" => 0x09,
        "enter" => 0x0D,
        "escape" => 0x1B,
        "up" => 0xF700,
        "down" => 0xF701,
        "left" => 0xF702,
        "right" => 0xF703,
        "delete" => 0xF728,
        "home" => 0xF729,
        "end" => 0xF72B,
        "pageup" => 0xF72C,
        "pagedown" => 0xF72D,
        _ => match n {
            Some(n @ 1..=12) => 0xF703 + n,
            _ => return None,
        },
    })
}

/// The control character other systems type, if any.
#[cfg(not(target_os = "macos"))]
pub fn character(key: &str) -> Option<u16> {
    Some(match key {
        "backspace" => 0x08,
        "tab" => 0x09,
        "enter" => 0x0D,
        "escape" => 0x1B,
        _ => return None,
    })
}

/// The chord for cut, copy, paste and select-all here.
pub fn edit_chord(modifiers: &Modifiers) -> bool {
    let primary = if cfg!(target_os = "macos") {
        modifiers.platform && !modifiers.control
    } else {
        modifiers.control && !modifiers.platform
    };
    primary && !modifiers.alt && !modifiers.shift
}

#[cfg(test)]
mod tests {
    use super::{Code, code};

    #[test]
    fn keys_carry_both_codes() {
        assert_eq!(
            code("a"),
            Some(Code {
                windows: 0x41,
                native: 0
            })
        );
        assert_eq!(
            code("z"),
            Some(Code {
                windows: 0x5A,
                native: 6
            })
        );
        assert_eq!(
            code("0"),
            Some(Code {
                windows: 0x30,
                native: 29
            })
        );
        assert_eq!(
            code("enter"),
            Some(Code {
                windows: 0x0D,
                native: 36
            })
        );
        assert_eq!(
            code("f1"),
            Some(Code {
                windows: 0x70,
                native: 122
            })
        );
        assert_eq!(
            code("f12"),
            Some(Code {
                windows: 0x7B,
                native: 111
            })
        );
        assert_eq!(
            code("["),
            Some(Code {
                windows: 0xDB,
                native: 33
            })
        );
        assert_eq!(code("f13"), None);
        assert_eq!(code("é"), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn keys_without_text_carry_appkit_characters() {
        assert_eq!(super::character("backspace"), Some(0x7F));
        assert_eq!(super::character("up"), Some(0xF700));
        assert_eq!(super::character("f1"), Some(0xF704));
        assert_eq!(super::character("f12"), Some(0xF70F));
        assert_eq!(super::character("a"), None);
    }
}
