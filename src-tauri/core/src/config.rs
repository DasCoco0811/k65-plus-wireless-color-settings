//! Settings: colors, timings, zones and special keys, plus the library of
//! custom color sets and profiles. Everything is stored as JSON.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Palette {
    /// Base color of all keys.
    pub base: String,
    /// Ring 1: around a pressed key.
    pub primary: String,
    /// Ring 2: alternates with ring 1 when colored keys are pressed.
    pub secondary: String,
}

impl Default for Palette {
    fn default() -> Self {
        // "Shibuya": neon violet, hot pink and cyan teal, picked on the real
        // keyboard. All three tones are far enough apart (OKLab ≥ 0.2) to
        // stay distinguishable as LED light too.
        Palette {
            base: "#9722B5".into(),
            primary: "#FF2172".into(),
            secondary: "#00E0D0".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ZoneConfig {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    /// LED names as in the SDK, e.g. `"Q"`, `"F5"`, `"NonUsTilde"`.
    pub keys: Vec<String>,
    pub own_colors: bool,
    pub palette: Palette,
}

impl Default for ZoneConfig {
    fn default() -> Self {
        ZoneConfig {
            id: String::new(),
            name: String::new(),
            enabled: true,
            keys: Vec::new(),
            own_colors: false,
            palette: Palette::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub palette: Palette,
    /// 0–100 %.
    pub brightness: u8,
    /// After this many seconds without use, a zone returns to the base color.
    pub timeout_secs: u32,
    /// 1 = direct neighbors, 2 = their neighbors as well.
    pub reach: u8,
    pub press_fade_ms: u32,
    pub reset_fade_ms: u32,
    /// For keyboards in Mac mode that swap Alt and Win/Cmd themselves.
    pub swap_alt_gui: bool,
    /// Convert screen colors to linear LED light (see [`to_led`]).
    pub led_correction: bool,
    pub paused: bool,
    pub zones: Vec<ZoneConfig>,
    /// Special keys: LED name → action instead of the normal ring. Modifiers
    /// only trigger it when tapped on their own.
    pub key_actions: BTreeMap<String, Action>,
    /// Key combinations like `"Gui+Backspace"` (see [`crate::combo`]) → action.
    pub combo_actions: BTreeMap<String, Action>,
    /// Learned media/special keys: identifier (e.g. `"VolumeUp"`) → LED name;
    /// empty name = deliberately ignored.
    pub learned_keys: BTreeMap<String, String>,
    /// Background effect while no key is colored.
    pub ambient: Ambient,
    /// Whether start-at-login was switched on once. After that the user's
    /// choice in the app counts and it is never switched on again.
    pub autostart_set_up: bool,
}

/// Background effects made from the base color, ring 1 and ring 2.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AmbientMode {
    /// Static base color.
    Off,
    /// Two color waves that slowly flow into each other.
    #[default]
    Aurora,
    /// A color gradient travels across the keyboard.
    Wave,
    /// The whole keyboard slowly fades through the three colors.
    Fade,
    /// Gentle dimming up and down.
    Breathe,
    /// Single keys calmly glow up and fade out again.
    Glimmer,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Ambient {
    pub mode: AmbientMode,
    /// Speed factor, 1 = normal.
    pub speed: f32,
    /// 0–100 %: how far the effect departs from the base color.
    pub intensity: u8,
}

impl Default for Ambient {
    fn default() -> Self {
        Ambient { mode: AmbientMode::default(), speed: 1.0, intensity: 80 }
    }
}

/// What a special key triggers instead of the normal ring. Zones are given by
/// their `id`. Effects briefly run across the whole keyboard and leave the
/// colors underneath unchanged.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Normal ring like any other key.
    Ring,
    ResetZones { zones: Vec<String> },
    ResetAll,
    /// Every key in the zones moves one color on: base → ring 1 → ring 2 → base.
    CycleZones { zones: Vec<String> },
    /// Ring 1 spreads out from the key across the keyboard.
    Wave,
    /// A stripe in ring 2 runs from left to right.
    Sweep,
    /// The whole keyboard flashes in ring 2.
    Flash,
    /// Random keys briefly sparkle in ring 1 and ring 2.
    Sparkle,
    /// A stripe rises from the bottom row to the top, turning from ring 1 to ring 2.
    Rise,
    /// A stripe in ring 1 wipes from right to left, resetting the zones key
    /// by key as it goes (empty = effect only).
    Wipe { zones: Vec<String> },
}

impl Default for Config {
    fn default() -> Self {
        Config {
            palette: Palette::default(),
            brightness: 100,
            timeout_secs: 60,
            reach: 1,
            press_fade_ms: 70,
            reset_fade_ms: 900,
            swap_alt_gui: false,
            led_correction: true,
            paused: false,
            zones: default_zones(),
            key_actions: default_key_actions(),
            combo_actions: default_combo_actions(),
            learned_keys: BTreeMap::new(),
            ambient: Ambient::default(),
            autostart_set_up: false,
        }
    }
}

/// ⌘ is written as `Gui`, ⌥ as `Alt`. Deleting a word is ⌥ + ⌫ on the Mac,
/// Ctrl + ⌫ on Windows; deleting a whole line is ⌘ + ⌫.
pub fn default_combo_actions() -> BTreeMap<String, Action> {
    let erase = || Action::Wipe { zones: vec!["letters".into(), "numbers".into()] };
    let swipe = || Action::Wipe { zones: Vec::new() };
    [
        ("Gui+Backspace", erase()),
        ("Alt+Backspace", swipe()),
        ("Ctrl+Backspace", swipe()),
        ("Gui+Space", Action::Wave),
        ("Gui+Tab", Action::Sweep),
        ("Alt+Tab", Action::Sweep),
    ]
    .into_iter()
    .map(|(k, a)| (k.to_string(), a))
    .collect()
}

pub fn default_key_actions() -> BTreeMap<String, Action> {
    let typing = || Action::ResetZones { zones: vec!["letters".into(), "numbers".into()] };
    [
        ("Space", typing()),
        ("Enter", typing()),
        ("Backspace", Action::CycleZones { zones: vec!["letters".into(), "numbers".into()] }),
        ("Escape", Action::ResetAll),
        ("Tab", Action::Sweep),
        ("CapsLock", Action::Flash),
        ("LeftAlt", Action::Wave),
        ("RightAlt", Action::Wave),
        ("LeftCtrl", Action::Wave),
        ("RightCtrl", Action::Wave),
        ("LeftGui", Action::Sparkle),
        ("RightGui", Action::Sparkle),
        ("LeftShift", Action::Rise),
        ("RightShift", Action::Rise),
    ]
    .into_iter()
    .map(|(k, a)| (k.to_string(), a))
    .collect()
}

fn zone(id: &str, name: &str, keys: &[&str]) -> ZoneConfig {
    ZoneConfig {
        id: id.into(),
        name: name.into(),
        keys: keys.iter().map(|k| k.to_string()).collect(),
        ..ZoneConfig::default()
    }
}

/// Zones following the layout of the K65 PLUS; keys a keyboard doesn't have
/// are ignored, so the same zones also fit larger models.
pub fn default_zones() -> Vec<ZoneConfig> {
    vec![
        zone(
            "letters",
            "Letters",
            &[
                "Tab", "Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "BracketLeft",
                "BracketRight", "Enter", "CapsLock", "A", "S", "D", "F", "G", "H", "J", "K", "L",
                "SemicolonAndColon", "ApostropheAndDoubleQuote", "NonUsTilde", "Backslash",
                "LeftShift", "NonUsBackslash", "Z", "X", "C", "V", "B", "N", "M",
                "CommaAndLessThan", "PeriodAndBiggerThan", "SlashAndQuestionMark", "RightShift",
            ],
        ),
        zone(
            "frow",
            "F-row",
            &[
                "Escape", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11",
                "F12", "Delete", "PrintScreen", "ScrollLock", "PauseBreak",
            ],
        ),
        zone(
            "numbers",
            "Numbers",
            &[
                "GraveAccentAndTilde", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0",
                "MinusAndUnderscore", "EqualsAndPlus", "Backspace",
            ],
        ),
        zone("nav", "Navigation", &["Insert", "Home", "PageUp", "End", "PageDown"]),
        zone("arrows", "Arrows", &["UpArrow", "LeftArrow", "DownArrow", "RightArrow"]),
        zone(
            "bottom",
            "Bottom row",
            &["LeftCtrl", "LeftGui", "LeftAlt", "RightAlt", "Fn", "RightGui", "Application", "RightCtrl"],
        ),
        zone(
            "numpad",
            "Numpad",
            &[
                "NumLock", "KeypadSlash", "KeypadAsterisk", "KeypadMinus", "Keypad7", "Keypad8",
                "Keypad9", "KeypadPlus", "Keypad4", "Keypad5", "Keypad6", "Keypad1", "Keypad2",
                "Keypad3", "KeypadEnter", "Keypad0", "KeypadPeriodAndDelete", "KeypadComma",
            ],
        ),
    ]
}

/// Custom color set with a name.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NamedPalette {
    pub name: String,
    pub palette: Palette,
}

/// Saved complete set of settings. The `id` stays the same when renaming.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub config: Config,
}

/// Custom color sets and profiles. Kept separate from the active
/// configuration, so loading a profile doesn't overwrite the custom sets.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Library {
    pub palettes: Vec<NamedPalette>,
    pub profiles: Vec<Profile>,
    /// Profile that changes are saved to automatically.
    pub active_profile: Option<String>,
}

/// Identifier in export files, so unrelated JSON files are rejected.
pub const EXPORT_FORMAT: &str = "k65-plus-wireless-color-settings";

/// File for taking along or sharing profiles and color sets.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ExportFile {
    pub format: String,
    pub version: u32,
    pub profiles: Vec<Profile>,
    pub palettes: Vec<NamedPalette>,
}

impl Library {
    pub fn export(&self) -> ExportFile {
        ExportFile {
            format: EXPORT_FORMAT.into(),
            version: 1,
            profiles: self.profiles.clone(),
            palettes: self.palettes.clone(),
        }
    }

    /// Adds profiles and color sets from an export file. Profiles get new IDs,
    /// duplicate names a suffix; identical color sets are skipped. Returns how
    /// many profiles and sets were added.
    pub fn import(&mut self, file: ExportFile, id_seed: u64) -> Result<(usize, usize), String> {
        if file.format != EXPORT_FORMAT {
            return Err("This is not a K65 Plus Wireless Color Settings profile file".into());
        }
        let unique = |taken: &dyn Fn(&str) -> bool, name: &str| -> String {
            if !taken(name) {
                return name.to_string();
            }
            (1..)
                .map(|n| if n == 1 { format!("{name} (imported)") } else { format!("{name} (imported {n})") })
                .find(|candidate| !taken(candidate))
                .unwrap()
        };
        let mut profiles = 0;
        for (i, mut p) in file.profiles.into_iter().enumerate() {
            p.name = unique(&|n| self.profiles.iter().any(|q| q.name == n), &p.name);
            p.id = format!("p-imp-{id_seed:x}-{i}");
            p.config.paused = false;
            self.profiles.push(p);
            profiles += 1;
        }
        let mut palettes = 0;
        for mut set in file.palettes {
            if self.palettes.contains(&set) {
                continue;
            }
            set.name = unique(&|n| self.palettes.iter().any(|q| q.name == n), &set.name);
            self.palettes.push(set);
            palettes += 1;
        }
        Ok((profiles, palettes))
    }
}

/// Reads a JSON file; if it is missing or unreadable, the defaults apply.
pub fn load<T: DeserializeOwned + Default>(path: &Path) -> T {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).map_err(|e| log::warn!("{} unreadable: {e}", path.display())).ok())
        .unwrap_or_default()
}

pub fn save<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(tmp, path)
}

/// `#RRGGBB` → RGB; invalid values become black.
pub fn parse_hex(hex: &str) -> [u8; 3] {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 {
        return [0, 0, 0];
    }
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0);
    [byte(0), byte(2), byte(4)]
}

/// Screen color (sRGB) → LED drive value. Screen values are gamma-encoded for
/// the eye, LEDs light up linearly to the value: without conversion, small
/// components shine far too strongly (red turns pink, purple turns lavender).
pub fn to_led(c: [u8; 3]) -> [u8; 3] {
    c.map(|v| {
        let s = v as f32 / 255.0;
        let l = if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) };
        (l * 255.0).round() as u8
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parsing() {
        assert_eq!(parse_hex("#B066FF"), [0xB0, 0x66, 0xFF]);
        assert_eq!(parse_hex("00e5c8"), [0x00, 0xE5, 0xC8]);
        assert_eq!(parse_hex("nope"), [0, 0, 0]);
    }

    #[test]
    fn led_correction_removes_tints() {
        // Old red: 30 green and 45 blue visibly added a pink tint on the LED.
        assert_eq!(to_led([255, 30, 45]), [255, 3, 7]);
        assert_eq!(to_led([0, 0, 0]), [0, 0, 0]);
        assert_eq!(to_led([255, 255, 255]), [255, 255, 255]);
    }

    #[test]
    fn old_config_without_actions_gets_defaults() {
        let json = r#"{"zones": [{"id": "letters", "name": "Letters", "keys": ["Q"], "reset_keys": ["Space"]}]}"#;
        let cfg: Config = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.key_actions, default_key_actions());
        assert_eq!(cfg.zones[0].keys, ["Q"]);
    }

    #[test]
    fn library_roundtrip() {
        let dir = std::env::temp_dir().join(format!("k65-color-test-{}", std::process::id()));
        let path = dir.join("library.json");
        let lib = Library {
            palettes: vec![NamedPalette { name: "My set".into(), palette: Palette::default() }],
            profiles: vec![Profile {
                id: "p-1".into(),
                name: "Evenings".into(),
                config: Config { brightness: 40, ..Config::default() },
            }],
            active_profile: Some("p-1".into()),
        };
        save(&path, &lib).unwrap();
        assert_eq!(load::<Library>(&path), lib);
        assert_eq!(load::<Library>(&dir.join("missing.json")), Library::default());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn import_adds_with_fresh_ids_and_unique_names() {
        let mut lib = Library {
            palettes: vec![NamedPalette { name: "Purple".into(), palette: Palette::default() }],
            profiles: vec![Profile { id: "p-1".into(), name: "Evening".into(), config: Config::default() }],
            active_profile: Some("p-1".into()),
        };
        let file = lib.export();
        assert_eq!(lib.import(file, 0xab).unwrap(), (1, 0), "the same set is not added twice");
        assert_eq!(lib.profiles[1].name, "Evening (imported)");
        assert_eq!(lib.profiles[1].id, "p-imp-ab-0");
        assert_eq!(lib.active_profile.as_deref(), Some("p-1"), "active profile stays");

        let foreign = ExportFile { format: "something else".into(), ..ExportFile::default() };
        assert!(lib.import(foreign, 1).is_err());
    }

    #[test]
    fn partial_json_falls_back_to_defaults() {
        let cfg: Config = serde_json::from_str(r#"{"timeout_secs": 30}"#).unwrap();
        assert_eq!(cfg.timeout_secs, 30);
        assert_eq!(cfg.zones.len(), default_zones().len());
        assert_eq!(cfg.palette, Palette::default());
    }
}
