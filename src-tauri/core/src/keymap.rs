//! Operating system key codes → Corsair LED names.
//!
//! Both tables are position-based (not character-based), so Y/Z and umlauts
//! on German layouts light up in the right place.

/// macOS virtual keycode (`kVK_*`). On ISO keyboards macOS swaps the codes
/// for `^` (left of 1) and `<` (next to left Shift).
pub fn mac_keycode(code: u16, iso: bool) -> Option<&'static str> {
    Some(match code {
        0x00 => "A",
        0x01 => "S",
        0x02 => "D",
        0x03 => "F",
        0x04 => "H",
        0x05 => "G",
        0x06 => "Z",
        0x07 => "X",
        0x08 => "C",
        0x09 => "V",
        0x0A if iso => "GraveAccentAndTilde",
        0x0A => "NonUsBackslash",
        0x0B => "B",
        0x0C => "Q",
        0x0D => "W",
        0x0E => "E",
        0x0F => "R",
        0x10 => "Y",
        0x11 => "T",
        0x12 => "1",
        0x13 => "2",
        0x14 => "3",
        0x15 => "4",
        0x16 => "6",
        0x17 => "5",
        0x18 => "EqualsAndPlus",
        0x19 => "9",
        0x1A => "7",
        0x1B => "MinusAndUnderscore",
        0x1C => "8",
        0x1D => "0",
        0x1E => "BracketRight",
        0x1F => "O",
        0x20 => "U",
        0x21 => "BracketLeft",
        0x22 => "I",
        0x23 => "P",
        0x24 => "Enter",
        0x25 => "L",
        0x26 => "J",
        0x27 => "ApostropheAndDoubleQuote",
        0x28 => "K",
        0x29 => "SemicolonAndColon",
        0x2A => "NonUsTilde",
        0x2B => "CommaAndLessThan",
        0x2C => "SlashAndQuestionMark",
        0x2D => "N",
        0x2E => "M",
        0x2F => "PeriodAndBiggerThan",
        0x30 => "Tab",
        0x31 => "Space",
        0x32 if iso => "NonUsBackslash",
        0x32 => "GraveAccentAndTilde",
        0x33 => "Backspace",
        0x35 => "Escape",
        0x36 => "RightGui",
        0x37 => "LeftGui",
        0x38 => "LeftShift",
        0x39 => "CapsLock",
        0x3A => "LeftAlt",
        0x3B => "LeftCtrl",
        0x3C => "RightShift",
        0x3D => "RightAlt",
        0x3E => "RightCtrl",
        0x3F => "Fn",
        0x41 => "KeypadPeriodAndDelete",
        0x43 => "KeypadAsterisk",
        0x45 => "KeypadPlus",
        0x47 => "NumLock",
        0x4B => "KeypadSlash",
        0x4C => "KeypadEnter",
        0x4E => "KeypadMinus",
        0x52 => "Keypad0",
        0x53 => "Keypad1",
        0x54 => "Keypad2",
        0x55 => "Keypad3",
        0x56 => "Keypad4",
        0x57 => "Keypad5",
        0x58 => "Keypad6",
        0x59 => "Keypad7",
        0x5B => "Keypad8",
        0x5C => "Keypad9",
        0x5F => "KeypadComma",
        0x60 => "F5",
        0x61 => "F6",
        0x62 => "F7",
        0x63 => "F3",
        0x64 => "F8",
        0x65 => "F9",
        0x67 => "F11",
        0x69 => "PrintScreen",
        0x6B => "ScrollLock",
        0x6D => "F10",
        0x6F => "F12",
        0x71 => "PauseBreak",
        0x72 => "Insert",
        0x73 => "Home",
        0x74 => "PageUp",
        0x75 => "Delete",
        0x76 => "F4",
        0x77 => "End",
        0x78 => "F2",
        0x79 => "PageDown",
        0x7A => "F1",
        0x7B => "LeftArrow",
        0x7C => "RightArrow",
        0x7D => "DownArrow",
        0x7E => "UpArrow",
        _ => return None,
    })
}

/// Keys that produce text (letters, digits, punctuation, space).
/// The key diagnostics never record them.
pub fn is_character_key(name: &str) -> bool {
    name.chars().count() == 1
        || (name.starts_with("Keypad") && name.chars().count() == 7)
        || matches!(
            name,
            "Space"
                | "SemicolonAndColon"
                | "ApostropheAndDoubleQuote"
                | "BracketLeft"
                | "BracketRight"
                | "CommaAndLessThan"
                | "PeriodAndBiggerThan"
                | "SlashAndQuestionMark"
                | "MinusAndUnderscore"
                | "EqualsAndPlus"
                | "GraveAccentAndTilde"
                | "NonUsTilde"
                | "NonUsBackslash"
                | "Backslash"
        )
}

/// Special keys that macOS reports as a normal key without its own LED
/// (Apple functions on the F-row), otherwise the raw code.
pub fn mac_special_keycode(code: u16) -> String {
    match code {
        0xA0 => "MissionControl".into(),
        0x83 => "Launchpad".into(),
        0xB1 => "Spotlight".into(),
        0xB0 => "Dictation".into(),
        0xB2 => "DoNotDisturb".into(),
        _ => format!("mac:{code:#04x}"),
    }
}

/// Media key from a macOS system event (`NX_KEYTYPE_*`). Caps Lock and Num
/// Lock also arrive as normal keys and are skipped here.
pub fn mac_media_key(nx: u32) -> Option<String> {
    Some(match nx {
        0 => "VolumeUp".into(),
        1 => "VolumeDown".into(),
        2 => "BrightnessUp".into(),
        3 => "BrightnessDown".into(),
        4 | 10 => return None,
        7 => "Mute".into(),
        14 => "Eject".into(),
        16 => "PlayPause".into(),
        17 => "Next".into(),
        18 => "Previous".into(),
        19 => "Fast".into(),
        20 => "Rewind".into(),
        21 => "IlluminationUp".into(),
        22 => "IlluminationDown".into(),
        23 => "IlluminationToggle".into(),
        other => format!("nx:{other}"),
    })
}

/// Windows keys without their own LED: media keys by name, otherwise the raw scan code.
pub fn win_other_key(scan: u32, extended: bool) -> String {
    match (scan, extended) {
        (0x20, true) => "Mute".into(),
        (0x2E, true) => "VolumeDown".into(),
        (0x30, true) => "VolumeUp".into(),
        (0x22, true) => "PlayPause".into(),
        (0x24, true) => "Stop".into(),
        (0x19, true) => "Next".into(),
        (0x10, true) => "Previous".into(),
        _ => format!("win:{scan:#04x}{}", if extended { "e" } else { "" }),
    }
}

/// Windows scan code set 1 from `KBDLLHOOKSTRUCT` (`extended` = `LLKHF_EXTENDED`).
pub fn win_scancode(scan: u32, extended: bool) -> Option<&'static str> {
    if extended {
        return Some(match scan {
            0x1C => "KeypadEnter",
            0x1D => "RightCtrl",
            0x35 => "KeypadSlash",
            0x37 => "PrintScreen",
            0x38 => "RightAlt",
            0x45 => "NumLock",
            0x47 => "Home",
            0x48 => "UpArrow",
            0x49 => "PageUp",
            0x4B => "LeftArrow",
            0x4D => "RightArrow",
            0x4F => "End",
            0x50 => "DownArrow",
            0x51 => "PageDown",
            0x52 => "Insert",
            0x53 => "Delete",
            0x5B => "LeftGui",
            0x5C => "RightGui",
            0x5D => "Application",
            _ => return None,
        });
    }
    Some(match scan {
        0x01 => "Escape",
        0x02 => "1",
        0x03 => "2",
        0x04 => "3",
        0x05 => "4",
        0x06 => "5",
        0x07 => "6",
        0x08 => "7",
        0x09 => "8",
        0x0A => "9",
        0x0B => "0",
        0x0C => "MinusAndUnderscore",
        0x0D => "EqualsAndPlus",
        0x0E => "Backspace",
        0x0F => "Tab",
        0x10 => "Q",
        0x11 => "W",
        0x12 => "E",
        0x13 => "R",
        0x14 => "T",
        0x15 => "Y",
        0x16 => "U",
        0x17 => "I",
        0x18 => "O",
        0x19 => "P",
        0x1A => "BracketLeft",
        0x1B => "BracketRight",
        0x1C => "Enter",
        0x1D => "LeftCtrl",
        0x1E => "A",
        0x1F => "S",
        0x20 => "D",
        0x21 => "F",
        0x22 => "G",
        0x23 => "H",
        0x24 => "J",
        0x25 => "K",
        0x26 => "L",
        0x27 => "SemicolonAndColon",
        0x28 => "ApostropheAndDoubleQuote",
        0x29 => "GraveAccentAndTilde",
        0x2A => "LeftShift",
        0x2B => "NonUsTilde",
        0x2C => "Z",
        0x2D => "X",
        0x2E => "C",
        0x2F => "V",
        0x30 => "B",
        0x31 => "N",
        0x32 => "M",
        0x33 => "CommaAndLessThan",
        0x34 => "PeriodAndBiggerThan",
        0x35 => "SlashAndQuestionMark",
        0x36 => "RightShift",
        0x37 => "KeypadAsterisk",
        0x38 => "LeftAlt",
        0x39 => "Space",
        0x3A => "CapsLock",
        0x3B => "F1",
        0x3C => "F2",
        0x3D => "F3",
        0x3E => "F4",
        0x3F => "F5",
        0x40 => "F6",
        0x41 => "F7",
        0x42 => "F8",
        0x43 => "F9",
        0x44 => "F10",
        0x45 => "PauseBreak",
        0x46 => "ScrollLock",
        0x47 => "Keypad7",
        0x48 => "Keypad8",
        0x49 => "Keypad9",
        0x4A => "KeypadMinus",
        0x4B => "Keypad4",
        0x4C => "Keypad5",
        0x4D => "Keypad6",
        0x4E => "KeypadPlus",
        0x4F => "Keypad1",
        0x50 => "Keypad2",
        0x51 => "Keypad3",
        0x52 => "Keypad0",
        0x53 => "KeypadPeriodAndDelete",
        0x56 => "NonUsBackslash",
        0x57 => "F11",
        0x58 => "F12",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::luid_for_name;

    #[test]
    fn every_mapped_name_is_a_real_led() {
        for code in 0..=0x7F {
            for iso in [false, true] {
                if let Some(n) = mac_keycode(code, iso) {
                    assert!(luid_for_name(n).is_some(), "mac {code:#x} → {n}");
                }
            }
        }
        for scan in 0..=0x7F {
            for ext in [false, true] {
                if let Some(n) = win_scancode(scan, ext) {
                    assert!(luid_for_name(n).is_some(), "win {scan:#x} → {n}");
                }
            }
        }
    }

    #[test]
    fn character_keys_are_recognized() {
        for k in ["A", "7", "Space", "SemicolonAndColon", "Keypad4"] {
            assert!(is_character_key(k), "{k}");
        }
        for k in ["F5", "Escape", "LeftShift", "Backspace", "KeypadEnter", "UpArrow"] {
            assert!(!is_character_key(k), "{k}");
        }
    }

    #[test]
    fn german_positions() {
        // On the German layout Z sits where Y is and vice versa – we work position-based.
        assert_eq!(mac_keycode(0x10, true), Some("Y"));
        assert_eq!(mac_keycode(0x0A, true), Some("GraveAccentAndTilde"));
        assert_eq!(mac_keycode(0x32, true), Some("NonUsBackslash"));
        assert_eq!(win_scancode(0x56, false), Some("NonUsBackslash"));
        assert_eq!(win_scancode(0x38, true), Some("RightAlt"));
    }
}
