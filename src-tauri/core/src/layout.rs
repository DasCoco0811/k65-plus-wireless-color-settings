//! Physical keyboard layout: LED names, key sizes and neighborhoods.
//!
//! The iCUE SDK only provides the center of each LED in millimeters. We add
//! the key widths from standard sizes and shorten them where they would
//! overlap the neighboring key in the same row (e.g. right Shift on 75%
//! keyboards).

use serde::Serialize;

/// One key unit (1u) in millimeters.
pub const UNIT_MM: f64 = 19.05;

/// Group of the regular keyboard LEDs in the SDK (`CLG_Keyboard`).
pub const GROUP_KEYBOARD: u32 = 0;

/// `CorsairLedId_Keyboard` from `iCUESDKLedIdEnum.h` (SDK 4.0.84).
const KEY_NAMES: &[(u32, &str)] = &[
    (1, "Escape"), (2, "F1"), (3, "F2"), (4, "F3"), (5, "F4"), (6, "F5"), (7, "F6"),
    (8, "F7"), (9, "F8"), (10, "F9"), (11, "F10"), (12, "F11"), (13, "F12"),
    (14, "GraveAccentAndTilde"), (15, "1"), (16, "2"), (17, "3"), (18, "4"), (19, "5"),
    (20, "6"), (21, "7"), (22, "8"), (23, "9"), (24, "0"), (25, "MinusAndUnderscore"),
    (26, "EqualsAndPlus"), (27, "Backspace"), (28, "Tab"), (29, "Q"), (30, "W"), (31, "E"),
    (32, "R"), (33, "T"), (34, "Y"), (35, "U"), (36, "I"), (37, "O"), (38, "P"),
    (39, "BracketLeft"), (40, "BracketRight"), (41, "CapsLock"), (42, "A"), (43, "S"),
    (44, "D"), (45, "F"), (46, "G"), (47, "H"), (48, "J"), (49, "K"), (50, "L"),
    (51, "SemicolonAndColon"), (52, "ApostropheAndDoubleQuote"), (53, "Backslash"),
    (54, "Enter"), (55, "LeftShift"), (56, "NonUsBackslash"), (57, "Z"), (58, "X"),
    (59, "C"), (60, "V"), (61, "B"), (62, "N"), (63, "M"), (64, "CommaAndLessThan"),
    (65, "PeriodAndBiggerThan"), (66, "SlashAndQuestionMark"), (67, "RightShift"),
    (68, "LeftCtrl"), (69, "LeftGui"), (70, "LeftAlt"), (71, "Space"), (72, "RightAlt"),
    (73, "RightGui"), (74, "Application"), (75, "RightCtrl"), (76, "LedProgramming"),
    (77, "Lang1"), (78, "Lang2"), (79, "International1"), (80, "International2"),
    (81, "International3"), (82, "International4"), (83, "International5"),
    (84, "PrintScreen"), (85, "ScrollLock"), (86, "PauseBreak"), (87, "Insert"),
    (88, "Home"), (89, "PageUp"), (90, "Delete"), (91, "End"), (92, "PageDown"),
    (93, "UpArrow"), (94, "LeftArrow"), (95, "DownArrow"), (96, "RightArrow"),
    (97, "NonUsTilde"), (98, "Brightness"), (99, "WinLock"), (100, "Mute"), (101, "Stop"),
    (102, "ScanPreviousTrack"), (103, "PlayPause"), (104, "ScanNextTrack"), (105, "NumLock"),
    (106, "KeypadSlash"), (107, "KeypadAsterisk"), (108, "KeypadMinus"), (109, "Keypad7"),
    (110, "Keypad8"), (111, "Keypad9"), (112, "KeypadPlus"), (113, "Keypad4"),
    (114, "Keypad5"), (115, "Keypad6"), (116, "Keypad1"), (117, "Keypad2"), (118, "Keypad3"),
    (119, "KeypadComma"), (120, "KeypadEnter"), (121, "Keypad0"),
    (122, "KeypadPeriodAndDelete"), (123, "VolumeUp"), (124, "VolumeDown"), (125, "MR"),
    (126, "M1"), (127, "M2"), (128, "M3"), (129, "Fn"),
];

/// LED positions of the K65 PLUS WIRELESS (ISO), read live via the SDK.
/// Serves as a preview while no keyboard is connected, and for tests.
const FALLBACK_K65_PLUS_ISO: &str = include_str!("../fixtures/k65_plus_iso.txt");

pub fn luid(group: u32, id: u32) -> u32 {
    (group << 16) | id
}

pub fn name_for_luid(luid: u32) -> String {
    let (group, id) = (luid >> 16, luid & 0xFFFF);
    if group == GROUP_KEYBOARD {
        if let Some((_, name)) = KEY_NAMES.iter().find(|(i, _)| *i == id) {
            return (*name).to_string();
        }
    }
    format!("g{group}:{id}")
}

pub fn luid_for_name(name: &str) -> Option<u32> {
    KEY_NAMES
        .iter()
        .find(|(_, n)| *n == name)
        .map(|(id, _)| luid(GROUP_KEYBOARD, *id))
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Led {
    pub luid: u32,
    pub name: String,
    /// Center and size in millimeters.
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct Layout {
    pub leds: Vec<Led>,
}

impl Layout {
    /// Builds the layout from SDK positions `(luid, x_mm, y_mm)`.
    pub fn from_positions(positions: &[(u32, f64, f64)]) -> Layout {
        let names: Vec<String> = positions.iter().map(|p| name_for_luid(p.0)).collect();
        let iso = names.iter().any(|n| n == "NonUsTilde" || n == "NonUsBackslash");
        let mut leds: Vec<Led> = positions
            .iter()
            .zip(names)
            .map(|(&(luid, x, y), name)| {
                let (wu, hu) = standard_size(&name, iso);
                Led { luid, name, x, y, w: wu * UNIT_MM, h: hu * UNIT_MM }
            })
            .collect();
        clamp_widths(&mut leds);
        Layout { leds }
    }

    pub fn fallback() -> Layout {
        let positions: Vec<(u32, f64, f64)> = FALLBACK_K65_PLUS_ISO
            .lines()
            .filter_map(|line| {
                let mut it = line.split_whitespace();
                Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?))
            })
            .collect();
        Layout::from_positions(&positions)
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.leds.iter().position(|l| l.name == name)
    }

    pub fn index_of_luid(&self, luid: u32) -> Option<usize> {
        self.leds.iter().position(|l| l.luid == luid)
    }

    /// Direct physical neighbors per LED (regardless of zones).
    pub fn adjacency(&self) -> Vec<Vec<usize>> {
        let n = self.leds.len();
        let mut adj = vec![Vec::new(); n];
        for a in 0..n {
            for b in (a + 1)..n {
                if touching(&self.leds[a], &self.leds[b]) {
                    adj[a].push(b);
                    adj[b].push(a);
                }
            }
        }
        adj
    }
}

/// Standard sizes in units (width, height). Everything else is 1u × 1u.
fn standard_size(name: &str, iso: bool) -> (f64, f64) {
    match name {
        "Backspace" => (2.0, 1.0),
        "Tab" => (1.5, 1.0),
        "Backslash" => (1.5, 1.0),
        "CapsLock" => (1.75, 1.0),
        "Enter" if iso => (1.25, 2.0),
        "Enter" => (2.25, 1.0),
        "LeftShift" if iso => (1.25, 1.0),
        "LeftShift" => (2.25, 1.0),
        "RightShift" => (2.75, 1.0),
        "LeftCtrl" | "LeftGui" | "LeftAlt" => (1.25, 1.0),
        "Space" => (6.25, 1.0),
        "Keypad0" => (2.0, 1.0),
        "KeypadPlus" | "KeypadEnter" => (1.0, 2.0),
        _ => (1.0, 1.0),
    }
}

/// Horizontal and vertical distance between key edges (negative = overlap).
fn gaps(a: &Led, b: &Led) -> (f64, f64) {
    let gx = (a.x - b.x).abs() - (a.w + b.w) / 2.0;
    let gy = (a.y - b.y).abs() - (a.h + b.h) / 2.0;
    (gx, gy)
}

/// Same row = clearly overlapping vertically.
fn same_row(gy: f64) -> bool {
    gy < -2.0
}

/// Shortens keys symmetrically that reach into the neighboring key in their row.
fn clamp_widths(leds: &mut [Led]) {
    for i in 0..leds.len() {
        let mut half = leds[i].w / 2.0;
        for j in 0..leds.len() {
            if i == j {
                continue;
            }
            let (_, gy) = gaps(&leds[i], &leds[j]);
            if !same_row(gy) {
                continue;
            }
            let dx = (leds[i].x - leds[j].x).abs();
            let room = dx - leds[j].w.min(UNIT_MM) / 2.0;
            if room > 1.0 && room < half {
                half = room;
            }
        }
        leds[i].w = half * 2.0;
    }
}

/// Two keys are adjacent: in the same row with a small gap (F-row blocks
/// count too), or in neighboring rows with horizontal overlap or touching
/// corners.
fn touching(a: &Led, b: &Led) -> bool {
    let (gx, gy) = gaps(a, b);
    if same_row(gy) {
        gx <= 7.0
    } else {
        gy <= 5.0 && gx <= 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn neighbors(layout: &Layout, name: &str) -> Vec<String> {
        let adj = layout.adjacency();
        let i = layout.index_of(name).unwrap();
        let mut v: Vec<String> = adj[i].iter().map(|&j| layout.leds[j].name.clone()).collect();
        v.sort();
        v
    }

    #[test]
    fn fallback_has_all_k65_leds() {
        let l = Layout::fallback();
        assert_eq!(l.leds.len(), 82);
        assert!(l.index_of("NonUsTilde").is_some());
        assert!(l.index_of("Fn").is_some());
    }

    #[test]
    fn letter_neighbors_follow_the_stagger() {
        let l = Layout::fallback();
        // D: left S, right F, above E/R, below X/C
        assert_eq!(neighbors(&l, "D"), ["C", "E", "F", "R", "S", "X"]);
    }

    #[test]
    fn wide_keys_get_their_neighbors() {
        let l = Layout::fallback();
        let caps = neighbors(&l, "CapsLock");
        for k in ["A", "Tab", "Q", "LeftShift", "NonUsBackslash"] {
            assert!(caps.contains(&k.to_string()), "CapsLock is missing {k}: {caps:?}");
        }
        let enter = neighbors(&l, "Enter");
        for k in ["BracketRight", "NonUsTilde", "Backspace", "RightShift"] {
            assert!(enter.contains(&k.to_string()), "Enter is missing {k}: {enter:?}");
        }
    }

    #[test]
    fn right_shift_is_clamped_on_75_percent() {
        let l = Layout::fallback();
        let rs = &l.leds[l.index_of("RightShift").unwrap()];
        assert!((rs.w / UNIT_MM - 1.75).abs() < 0.1, "RightShift {}u", rs.w / UNIT_MM);
    }

    #[test]
    fn f_row_blocks_touch_across_gaps() {
        let l = Layout::fallback();
        assert_eq!(neighbors(&l, "F4").iter().filter(|n| n.starts_with('F')).count(), 2);
        assert!(neighbors(&l, "Escape").contains(&"F1".to_string()));
    }

    #[test]
    fn arrow_cluster() {
        let l = Layout::fallback();
        let up = neighbors(&l, "UpArrow");
        for k in ["LeftArrow", "DownArrow", "RightArrow"] {
            assert!(up.contains(&k.to_string()), "UpArrow is missing {k}: {up:?}");
        }
    }
}
