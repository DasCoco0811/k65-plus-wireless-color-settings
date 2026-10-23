//! Key combinations: which modifiers are currently held and whether a
//! modifier was tapped on its own.
//!
//! Modifiers (Shift, Ctrl, Alt/Option, Win/Cmd, Fn) only trigger their own
//! action on release, and only if no other key was pressed in the meantime.
//! So ⌘ + ⌫ fires only the combination, not the effect of ⌘ on top of it.

use crate::input::{Key, KeyInput};

pub const SHIFT: u8 = 1;
pub const CTRL: u8 = 2;
pub const ALT: u8 = 4;
pub const GUI: u8 = 8;

/// Order and spelling in combination names like `"Gui+Backspace"`.
const NAMES: [(u8, &str); 4] = [(CTRL, "Ctrl"), (ALT, "Alt"), (SHIFT, "Shift"), (GUI, "Gui")];

fn modifier_bit(name: &str) -> Option<u8> {
    match name {
        "LeftShift" | "RightShift" => Some(SHIFT),
        "LeftCtrl" | "RightCtrl" => Some(CTRL),
        "LeftAlt" | "RightAlt" => Some(ALT),
        "LeftGui" | "RightGui" => Some(GUI),
        "Fn" => Some(0),
        _ => None,
    }
}

pub fn is_modifier(name: &str) -> bool {
    modifier_bit(name).is_some()
}

/// `"Ctrl+Alt+Shift+Gui+Key"`, with only the modifiers that are set.
pub fn combo_name(mods: u8, key: &str) -> String {
    let mut parts: Vec<&str> = NAMES.iter().filter(|(bit, _)| mods & bit != 0).map(|(_, n)| *n).collect();
    parts.push(key);
    parts.join("+")
}

/// Counterpart to [`combo_name`]; `None` if no modifier is included.
pub fn parse_combo(combo: &str) -> Option<(u8, &str)> {
    let mut mods = 0;
    let mut parts = combo.split('+').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            return (mods != 0 && !part.is_empty()).then_some((mods, part));
        }
        mods |= NAMES.iter().find(|(_, n)| *n == part)?.0;
    }
    None
}

/// Swaps the Alt and Gui bits (keyboards in Mac mode).
pub fn swap_alt_gui(mods: u8) -> u8 {
    let rest = mods & !(ALT | GUI);
    let alt = if mods & ALT != 0 { GUI } else { 0 };
    let gui = if mods & GUI != 0 { ALT } else { 0 };
    rest | alt | gui
}

#[derive(Default)]
pub struct ComboTracker {
    /// Held modifiers and whether they were already used for something else.
    held: Vec<(&'static str, bool)>,
}

impl ComboTracker {
    pub const fn new() -> Self {
        ComboTracker { held: Vec::new() }
    }

    fn mods(&self) -> u8 {
        self.held.iter().filter_map(|(n, _)| modifier_bit(n)).fold(0, |a, b| a | b)
    }

    fn mark_used(&mut self) {
        for (_, used) in &mut self.held {
            *used = true;
        }
    }

    /// Key pressed (without repeats). Returns what should be triggered.
    pub fn down(&mut self, key: Key) -> Option<KeyInput> {
        if let Key::Led(name) = key {
            if is_modifier(name) {
                if !self.held.iter().any(|(n, _)| *n == name) {
                    // Modifier chords (e.g. ⌘ + Shift) don't count as a tap for any of them.
                    let chord = !self.held.is_empty();
                    self.mark_used();
                    self.held.push((name, chord));
                }
                return None;
            }
        }
        let mods = self.mods();
        self.mark_used();
        Some(KeyInput { key, mods })
    }

    /// Key released. A modifier tapped on its own triggers now.
    pub fn up(&mut self, name: &'static str) -> Option<KeyInput> {
        let pos = self.held.iter().position(|(n, _)| *n == name)?;
        let (_, used) = self.held.remove(pos);
        (!used).then_some(KeyInput { key: Key::Led(name), mods: self.mods() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn led(name: &'static str, mods: u8) -> Option<KeyInput> {
        Some(KeyInput { key: Key::Led(name), mods })
    }

    #[test]
    fn names_roundtrip() {
        assert_eq!(combo_name(GUI, "Backspace"), "Gui+Backspace");
        assert_eq!(combo_name(CTRL | ALT | SHIFT | GUI, "Q"), "Ctrl+Alt+Shift+Gui+Q");
        assert_eq!(parse_combo("Alt+Tab"), Some((ALT, "Tab")));
        assert_eq!(parse_combo("Backspace"), None);
        assert_eq!(parse_combo("Hyper+Q"), None);
        assert_eq!(swap_alt_gui(ALT | SHIFT), GUI | SHIFT);
    }

    #[test]
    fn combo_fires_once_and_suppresses_the_modifier_tap() {
        let mut t = ComboTracker::default();
        assert_eq!(t.down(Key::Led("LeftGui")), None);
        assert_eq!(t.down(Key::Led("Backspace")), led("Backspace", GUI));
        assert_eq!(t.up("LeftGui"), None, "⌘ was used for the combo");
    }

    #[test]
    fn lone_modifier_tap_fires_on_release() {
        let mut t = ComboTracker::default();
        assert_eq!(t.down(Key::Led("LeftAlt")), None);
        assert_eq!(t.up("LeftAlt"), led("LeftAlt", 0));
    }

    #[test]
    fn shift_for_capitals_is_not_a_tap() {
        let mut t = ComboTracker::default();
        t.down(Key::Led("LeftShift"));
        assert_eq!(t.down(Key::Led("H")), led("H", SHIFT));
        assert_eq!(t.up("LeftShift"), None);
        assert_eq!(t.down(Key::Led("A")), led("A", 0));
    }

    #[test]
    fn modifier_chords_are_not_taps() {
        let mut t = ComboTracker::default();
        t.down(Key::Led("LeftGui"));
        t.down(Key::Led("LeftShift"));
        assert_eq!(t.up("LeftShift"), None);
        assert_eq!(t.up("LeftGui"), None);
    }

    #[test]
    fn media_keys_carry_modifiers_too() {
        let mut t = ComboTracker::default();
        t.down(Key::Led("LeftAlt"));
        let got = t.down(Key::Other("VolumeUp".into()));
        assert_eq!(got, Some(KeyInput { key: Key::Other("VolumeUp".into()), mods: ALT }));
    }
}
