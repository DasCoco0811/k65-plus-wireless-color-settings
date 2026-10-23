//! Light logic.
//!
//! Every key is in the base color, ring 1 (primary) or ring 2 (secondary).
//! A key press colors the ring (neighbors in the same zone):
//! - Neighbors in the base color get ring 1, or ring 2 if the pressed key
//!   was already ring 1 (and vice versa).
//! - Neighbors that are already colored move on: ring 1 → ring 2 → ring 1 …
//! - The pressed key itself returns to the base color.
//!
//! This keeps the colors moving while typing. A zone that hasn't been used
//! for a while fades back to the base color.
//!
//! Special keys trigger an action instead of the ring (reset zones, cycle
//! colors) or an effect that briefly runs across the whole keyboard without
//! changing the colors underneath.
//!
//! When the keyboard is "clean" (all base color), a background effect made
//! from the three colors fades in; the first colored ring makes it static
//! again.

use crate::combo;
use crate::config::{parse_hex, Action, AmbientMode, Config, Palette};
use crate::layout::Layout;
use serde::Serialize;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyState {
    Base,
    Primary,
    Secondary,
}

type Rgb = [f32; 3];

#[derive(Clone, Copy)]
struct Fade {
    from: Rgb,
    to: Rgb,
    start: Instant,
    dur: Duration,
}

impl Fade {
    fn still(c: Rgb, now: Instant) -> Fade {
        Fade { from: c, to: c, start: now, dur: Duration::ZERO }
    }

    fn progress(&self, now: Instant) -> f32 {
        if self.dur.is_zero() {
            return 1.0;
        }
        (now.saturating_duration_since(self.start).as_secs_f32() / self.dur.as_secs_f32()).min(1.0)
    }

    fn value(&self, now: Instant) -> Rgb {
        let t = self.progress(now);
        let e = t * t * (3.0 - 2.0 * t);
        [0, 1, 2].map(|c| self.from[c] + (self.to[c] - self.from[c]) * e)
    }

    fn done(&self, now: Instant) -> bool {
        self.progress(now) >= 1.0
    }
}

/// Smooth fade in and out of a value between 0 and 1.
#[derive(Clone, Copy)]
struct Level {
    from: f32,
    to: f32,
    start: Instant,
    dur: Duration,
}

impl Level {
    fn value(&self, now: Instant) -> f32 {
        let t = if self.dur.is_zero() {
            1.0
        } else {
            (now.saturating_duration_since(self.start).as_secs_f32() / self.dur.as_secs_f32()).min(1.0)
        };
        self.from + (self.to - self.from) * t * t * (3.0 - 2.0 * t)
    }
}

const AMBIENT_IN: Duration = Duration::from_millis(1200);
const AMBIENT_OUT: Duration = Duration::from_millis(250);
/// Frame rate during transitions and effects.
const FRAME_FAST: Duration = Duration::from_millis(16);
/// Frame rate when only the background is running: spares the keyboard's wireless link.
const FRAME_AMBIENT: Duration = Duration::from_millis(33);

fn mix(a: Rgb, b: Rgb, w: f32) -> Rgb {
    [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * w)
}

/// Point `u` on the color cycle base → ring 1 → ring 2 → base, smoothly blended.
fn palette_at(colors: &[Rgb; 3], u: f64) -> Rgb {
    let u = u.rem_euclid(1.0) * 3.0;
    let k = (u.floor() as usize).min(2);
    mix(colors[k], colors[(k + 1) % 3], smooth(u.fract()) as f32)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum EffectKind {
    Wave,
    Sweep,
    Flash,
    Sparkle,
    Wipe,
    Rise,
}

const WAVE_SPEED_MM_S: f64 = 320.0;
const WAVE_WIDTH_MM: f64 = 24.0;
const SWEEP_SECS: f64 = 0.75;
const SWEEP_WIDTH_MM: f64 = 34.0;
const WIPE_SECS: f64 = 0.55;
const WIPE_WIDTH_MM: f64 = 30.0;
const FLASH_SECS: f64 = 0.55;
const RISE_SECS: f64 = 0.6;
const RISE_HEIGHT_MM: f64 = 26.0;
const SPARKLE_SECS: f64 = 1.0;
const SPARKLE_KEY_SECS: f64 = 0.3;
/// Share of keys (in percent) that light up when sparkling.
const SPARKLE_SHARE: u64 = 35;
const MAX_EFFECTS: usize = 8;

struct Effect {
    kind: EffectKind,
    x: f64,
    y: f64,
    start: Instant,
    secs: f64,
    seed: u64,
}

/// Resolved action of a special key (zone IDs → indices).
#[derive(Clone)]
enum KeyAction {
    Ring,
    ResetZones(Vec<usize>),
    ResetAll,
    CycleZones(Vec<usize>),
    Effect(EffectKind),
    Wipe(Vec<usize>),
}

fn splitmix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn smooth(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn palette_rgb(p: &Palette) -> [Rgb; 3] {
    [&p.base, &p.primary, &p.secondary].map(|h| parse_hex(h).map(f32::from))
}

pub struct Engine {
    layout: Layout,
    cfg: Config,
    zone_of: Vec<Option<usize>>,
    zone_leds: Vec<Vec<usize>>,
    /// Per LED: action, if it is a special key.
    actions: Vec<Option<KeyAction>>,
    /// Key combinations: (modifier bits, LED, action).
    combos: Vec<(u8, usize, KeyAction)>,
    /// Resets scheduled by a wipe: (time, LED).
    pending: Vec<(Instant, usize)>,
    neighbors: Vec<Vec<usize>>,
    global_colors: [Rgb; 3],
    zone_colors: Vec<Option<[Rgb; 3]>>,
    state: Vec<KeyState>,
    last_used: Vec<Option<Instant>>,
    fades: Vec<Fade>,
    effects: Vec<Effect>,
    rng: u64,
    /// Visibility of the background effect (0 = static, 1 = full).
    ambient: Level,
    /// Time base of the background patterns, so they don't restart on every start.
    clock: Instant,
}

impl Engine {
    pub fn new(layout: Layout, cfg: Config, now: Instant) -> Engine {
        let n = layout.leds.len();
        let mut e = Engine {
            layout,
            cfg,
            zone_of: Vec::new(),
            zone_leds: Vec::new(),
            actions: Vec::new(),
            combos: Vec::new(),
            pending: Vec::new(),
            neighbors: Vec::new(),
            global_colors: [[0.0; 3]; 3],
            zone_colors: Vec::new(),
            state: vec![KeyState::Base; n],
            last_used: Vec::new(),
            fades: Vec::new(),
            effects: Vec::new(),
            rng: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(1),
            ambient: Level { from: 0.0, to: 0.0, start: now, dur: Duration::ZERO },
            clock: now,
        };
        e.rebuild();
        e.fades = (0..n).map(|i| Fade::still(e.target(i), now)).collect();
        e.update_ambient(now);
        e
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn config(&self) -> &Config {
        &self.cfg
    }

    pub fn states(&self) -> &[KeyState] {
        &self.state
    }

    pub fn zone_of(&self) -> &[Option<usize>] {
        &self.zone_of
    }

    pub fn set_config(&mut self, cfg: Config, now: Instant) {
        let old_zone_of = std::mem::take(&mut self.zone_of);
        self.cfg = cfg;
        self.rebuild();
        for (i, old) in old_zone_of.iter().enumerate() {
            if *old != self.zone_of[i] {
                self.state[i] = KeyState::Base;
            }
        }
        self.retarget_all(now, self.press_fade());
        self.update_ambient(now);
    }

    /// New device or new LED positions: state starts over.
    pub fn set_layout(&mut self, layout: Layout, now: Instant) {
        let n = layout.leds.len();
        self.layout = layout;
        self.state = vec![KeyState::Base; n];
        self.last_used.clear();
        self.effects.clear();
        self.pending.clear();
        self.rebuild();
        self.fades = (0..n).map(|i| Fade::still(self.target(i), now)).collect();
        self.update_ambient(now);
    }

    /// Clean = no key colored and no wipe in progress.
    fn is_clean(&self) -> bool {
        self.pending.is_empty() && self.state.iter().all(|s| *s == KeyState::Base)
    }

    /// Fades the background effect in as soon as the keyboard is clean, and
    /// quickly out again as soon as a key gets colored.
    fn update_ambient(&mut self, now: Instant) {
        let target = if self.cfg.ambient.mode != AmbientMode::Off && self.is_clean() { 1.0 } else { 0.0 };
        if self.ambient.to != target {
            let from = self.ambient.value(now);
            let dur = if target > 0.5 { AMBIENT_IN } else { AMBIENT_OUT };
            self.ambient = Level { from, to: target, start: now, dur };
        }
    }

    fn ambient_visible(&self, now: Instant) -> bool {
        self.ambient.value(now) > 0.0 || self.ambient.to > 0.0
    }

    /// Background color of a key after `t` (speed-scaled) seconds.
    fn ambient_color(&self, i: usize, t: f64, bounds: (f64, f64, f64, f64)) -> Rgb {
        let led = &self.layout.leds[i];
        let colors = self.zone_of[i].and_then(|z| self.zone_colors[z]).unwrap_or(self.global_colors);
        let (x0, x1, y0, y1) = bounds;
        let nx = (led.x - x0) / (x1 - x0).max(1.0);
        let ny = (led.y - y0) / (y1 - y0).max(1.0);
        match self.cfg.ambient.mode {
            AmbientMode::Off => colors[0],
            AmbientMode::Aurora => {
                let a = palette_at(&colors, nx * 0.7 - t * 0.07);
                let b = palette_at(&colors, ny * 0.6 + nx * 0.3 + t * 0.05 + 0.33);
                let w = 0.5 + 0.5 * (t * 0.35 + nx * 4.0 - ny * 2.0).sin();
                mix(a, b, w as f32)
            }
            AmbientMode::Wave => palette_at(&colors, nx * 0.9 + ny * 0.25 - t * 0.12),
            AmbientMode::Fade => palette_at(&colors, t * 0.05),
            AmbientMode::Breathe => {
                let c = palette_at(&colors, t * 0.03);
                let level = 0.35 + 0.65 * (0.5 + 0.5 * (t * 1.1).sin());
                c.map(|v| v * level as f32)
            }
            AmbientMode::Glimmer => {
                let h = splitmix(i as u64 ^ 0x5EED);
                let period = 5.0 + (h % 400) as f64 / 100.0;
                let phase = ((h >> 12) % 1000) as f64 / 1000.0;
                let local = (t / period + phase).fract();
                let glow = if local < 0.3 { (std::f64::consts::PI * local / 0.3).sin() } else { 0.0 };
                let target = if (h >> 30) & 1 == 0 { colors[1] } else { colors[2] };
                mix(colors[0], target, (glow * 0.85) as f32)
            }
        }
    }

    fn bounds(&self) -> (f64, f64, f64, f64) {
        self.layout.leds.iter().fold((f64::MAX, f64::MIN, f64::MAX, f64::MIN), |(a, b, c, d), l| {
            (a.min(l.x), b.max(l.x), c.min(l.y), d.max(l.y))
        })
    }

    fn rebuild(&mut self) {
        let n = self.layout.leds.len();
        let zones = &self.cfg.zones;
        let mut zone_of = vec![None; n];
        let mut zone_leds = vec![Vec::new(); zones.len()];
        for (z, zone) in zones.iter().enumerate() {
            if !zone.enabled {
                continue;
            }
            for key in &zone.keys {
                if let Some(i) = self.layout.index_of(key) {
                    if zone_of[i].is_none() {
                        zone_of[i] = Some(z);
                        zone_leds[z].push(i);
                    }
                }
            }
        }

        let adj = self.layout.adjacency();
        let depth = self.cfg.reach.clamp(1, 3) as usize;
        self.neighbors = (0..n)
            .map(|i| match zone_of[i] {
                Some(z) => ring(i, depth, &adj, |j| zone_of[j] == Some(z)),
                None => Vec::new(),
            })
            .collect();

        self.global_colors = palette_rgb(&self.cfg.palette);
        self.zone_colors = zones
            .iter()
            .map(|z| z.own_colors.then(|| palette_rgb(&z.palette)))
            .collect();
        if self.last_used.len() != zones.len() {
            self.last_used = vec![None; zones.len()];
        }
        let zone_index = |ids: &[String]| -> Vec<usize> {
            ids.iter().filter_map(|id| zones.iter().position(|z| &z.id == id)).collect()
        };
        let resolve = |a: &Action| match a {
            Action::Ring => KeyAction::Ring,
            Action::ResetZones { zones } => KeyAction::ResetZones(zone_index(zones)),
            Action::ResetAll => KeyAction::ResetAll,
            Action::CycleZones { zones } => KeyAction::CycleZones(zone_index(zones)),
            Action::Wave => KeyAction::Effect(EffectKind::Wave),
            Action::Sweep => KeyAction::Effect(EffectKind::Sweep),
            Action::Flash => KeyAction::Effect(EffectKind::Flash),
            Action::Sparkle => KeyAction::Effect(EffectKind::Sparkle),
            Action::Rise => KeyAction::Effect(EffectKind::Rise),
            Action::Wipe { zones } => KeyAction::Wipe(zone_index(zones)),
        };
        self.actions = self.layout.leds.iter().map(|led| self.cfg.key_actions.get(&led.name).map(resolve)).collect();
        self.combos = self
            .cfg
            .combo_actions
            .iter()
            .filter_map(|(combo, action)| {
                let (mods, key) = combo::parse_combo(combo)?;
                Some((mods, self.layout.index_of(key)?, resolve(action)))
            })
            .collect();
        self.zone_of = zone_of;
        self.zone_leds = zone_leds;
    }

    fn press_fade(&self) -> Duration {
        Duration::from_millis(self.cfg.press_fade_ms as u64)
    }

    fn target(&self, i: usize) -> Rgb {
        let colors = self.zone_of[i]
            .and_then(|z| self.zone_colors[z])
            .unwrap_or(self.global_colors);
        match self.state[i] {
            KeyState::Base => colors[0],
            KeyState::Primary => colors[1],
            KeyState::Secondary => colors[2],
        }
    }

    fn set_state(&mut self, i: usize, s: KeyState, now: Instant, dur: Duration) {
        if self.state[i] == s {
            return;
        }
        self.state[i] = s;
        let from = self.fades[i].value(now);
        self.fades[i] = Fade { from, to: self.target(i), start: now, dur };
    }

    fn retarget_all(&mut self, now: Instant, dur: Duration) {
        for i in 0..self.fades.len() {
            let from = self.fades[i].value(now);
            self.fades[i] = Fade { from, to: self.target(i), start: now, dur };
        }
    }

    /// Resolves an LED name, including the Alt/Win swap and ISO/ANSI variants.
    fn resolve(&self, name: &str) -> Option<usize> {
        let name = match (self.cfg.swap_alt_gui, name) {
            (true, "LeftAlt") => "LeftGui",
            (true, "LeftGui") => "LeftAlt",
            (true, "RightAlt") => "RightGui",
            (true, "RightGui") => "RightAlt",
            _ => name,
        };
        self.layout.index_of(name).or_else(|| match name {
            "NonUsTilde" => self.layout.index_of("Backslash"),
            "Backslash" => self.layout.index_of("NonUsTilde"),
            _ => None,
        })
    }

    /// Handles a key press without modifiers. Returns the resolved LED name.
    pub fn press(&mut self, name: &str, now: Instant) -> Option<String> {
        self.press_combo(name, 0, now)
    }

    /// Handles a key press with held modifiers. If the combination has an
    /// action, only that runs; otherwise it acts like a normal press.
    /// Returns the LED name or the combination name.
    pub fn press_combo(&mut self, name: &str, mods: u8, now: Instant) -> Option<String> {
        let i = self.resolve(name)?;
        let mods = if self.cfg.swap_alt_gui { combo::swap_alt_gui(mods) } else { mods };
        let combo = (mods != 0)
            .then(|| self.combos.iter().find(|(m, led, _)| *m == mods && *led == i).cloned())
            .flatten();
        let name = match combo {
            Some((_, _, action)) => {
                self.run(Some(action), i, now);
                combo::combo_name(mods, &self.layout.leds[i].name)
            }
            None => {
                self.run(self.actions[i].clone(), i, now);
                self.layout.leds[i].name.clone()
            }
        };
        self.update_ambient(now);
        Some(name)
    }

    fn run(&mut self, action: Option<KeyAction>, i: usize, now: Instant) {
        match action {
            None | Some(KeyAction::Ring) => self.ring_press(i, now),
            Some(KeyAction::ResetZones(zones)) => {
                let quick = Duration::from_millis(self.cfg.press_fade_ms.max(180) as u64);
                for z in zones {
                    self.reset_zone(z, now, quick);
                }
            }
            Some(KeyAction::ResetAll) => self.reset_all(now),
            Some(KeyAction::CycleZones(zones)) => self.cycle_zones(&zones, now),
            Some(KeyAction::Effect(kind)) => self.start_effect(kind, i, now),
            Some(KeyAction::Wipe(zones)) => self.wipe(&zones, i, now),
        }
    }

    /// Wipe effect from right to left; keys in the zones switch to the base
    /// color exactly when the stripe reaches them.
    fn wipe(&mut self, zones: &[usize], i: usize, now: Instant) {
        self.start_effect(EffectKind::Wipe, i, now);
        let (x0, x1) = self.x_range();
        let travel = x1 - x0 + 2.0 * WIPE_WIDTH_MM;
        for &z in zones {
            for led in self.zone_leds[z].clone() {
                let secs = (x1 + WIPE_WIDTH_MM - self.layout.leds[led].x) / travel * WIPE_SECS;
                self.pending.push((now + Duration::from_secs_f64(secs.max(0.0)), led));
            }
            self.last_used[z] = None;
        }
    }

    fn x_range(&self) -> (f64, f64) {
        self.layout.leds.iter().fold((f64::MAX, f64::MIN), |(a, b), l| (a.min(l.x), b.max(l.x)))
    }

    fn ring_press(&mut self, i: usize, now: Instant) {
        if let Some(z) = self.zone_of[i] {
            let fade = self.press_fade();
            let fresh = match self.state[i] {
                KeyState::Base | KeyState::Secondary => KeyState::Primary,
                KeyState::Primary => KeyState::Secondary,
            };
            self.set_state(i, KeyState::Base, now, fade);
            for n in self.neighbors[i].clone() {
                let next = match self.state[n] {
                    KeyState::Base => fresh,
                    KeyState::Primary => KeyState::Secondary,
                    KeyState::Secondary => KeyState::Primary,
                };
                self.set_state(n, next, now, fade);
            }
            self.last_used[z] = Some(now);
        }
    }

    /// Every key in the zones moves one color on: base → ring 1 → ring 2 → base.
    fn cycle_zones(&mut self, zones: &[usize], now: Instant) {
        let fade = self.press_fade();
        for &z in zones {
            for i in self.zone_leds[z].clone() {
                let next = match self.state[i] {
                    KeyState::Base => KeyState::Primary,
                    KeyState::Primary => KeyState::Secondary,
                    KeyState::Secondary => KeyState::Base,
                };
                self.set_state(i, next, now, fade);
            }
            if !self.zone_leds[z].is_empty() {
                self.last_used[z] = Some(now);
            }
        }
    }

    fn start_effect(&mut self, kind: EffectKind, i: usize, now: Instant) {
        let (x, y) = (self.layout.leds[i].x, self.layout.leds[i].y);
        let secs = match kind {
            EffectKind::Wave => {
                let far = self.layout.leds.iter().map(|l| (l.x - x).hypot(l.y - y)).fold(0.0, f64::max);
                (far + WAVE_WIDTH_MM) / WAVE_SPEED_MM_S
            }
            EffectKind::Sweep => SWEEP_SECS,
            EffectKind::Wipe => WIPE_SECS,
            EffectKind::Flash => FLASH_SECS,
            EffectKind::Sparkle => SPARKLE_SECS,
            EffectKind::Rise => RISE_SECS,
        };
        self.rng = splitmix(self.rng);
        if self.effects.len() >= MAX_EFFECTS {
            self.effects.remove(0);
        }
        self.effects.push(Effect { kind, x, y, start: now, secs, seed: self.rng });
    }

    /// How strongly an effect covers LED `i` after `t` seconds, and with which color.
    fn overlay(&self, e: &Effect, i: usize, t: f64) -> Option<(f32, Rgb)> {
        if t < 0.0 || t >= e.secs {
            return None;
        }
        let led = &self.layout.leds[i];
        let [_, ring1, ring2] = self.global_colors;
        let (weight, color) = match e.kind {
            EffectKind::Wave => {
                let front = WAVE_SPEED_MM_S * t;
                let dist = (led.x - e.x).hypot(led.y - e.y);
                (smooth(1.0 - (dist - front).abs() / WAVE_WIDTH_MM), ring1)
            }
            EffectKind::Sweep => {
                let (x0, x1) = self.x_range();
                let pos = x0 - SWEEP_WIDTH_MM + (x1 - x0 + 2.0 * SWEEP_WIDTH_MM) * t / SWEEP_SECS;
                (smooth(1.0 - (led.x - pos).abs() / SWEEP_WIDTH_MM), ring2)
            }
            EffectKind::Wipe => {
                let (x0, x1) = self.x_range();
                let pos = x1 + WIPE_WIDTH_MM - (x1 - x0 + 2.0 * WIPE_WIDTH_MM) * t / WIPE_SECS;
                (smooth(1.0 - (led.x - pos).abs() / WIPE_WIDTH_MM), ring1)
            }
            EffectKind::Flash => ((1.0 - t / FLASH_SECS).powi(2), ring2),
            EffectKind::Rise => {
                let (_, _, y0, y1) = self.bounds();
                let progress = t / RISE_SECS;
                let pos = y1 + RISE_HEIGHT_MM - (y1 - y0 + 2.0 * RISE_HEIGHT_MM) * progress;
                (smooth(1.0 - (led.y - pos).abs() / RISE_HEIGHT_MM), mix(ring1, ring2, progress as f32))
            }
            EffectKind::Sparkle => {
                let h = splitmix(e.seed ^ i as u64);
                if h % 100 >= SPARKLE_SHARE {
                    return None;
                }
                let delay = ((h >> 8) % 1000) as f64 / 1000.0 * (SPARKLE_SECS - SPARKLE_KEY_SECS);
                let local = t - delay;
                if !(0.0..SPARKLE_KEY_SECS).contains(&local) {
                    return None;
                }
                let w = (std::f64::consts::PI * local / SPARKLE_KEY_SECS).sin();
                (w, if (h >> 20) & 1 == 0 { ring1 } else { ring2 })
            }
        };
        (weight > 0.0).then_some((weight as f32, color))
    }

    fn reset_zone(&mut self, z: usize, now: Instant, dur: Duration) {
        for i in self.zone_leds[z].clone() {
            self.set_state(i, KeyState::Base, now, dur);
        }
        self.last_used[z] = None;
    }

    pub fn reset_all(&mut self, now: Instant) {
        let dur = Duration::from_millis(300);
        for z in 0..self.zone_leds.len() {
            self.reset_zone(z, now, dur);
        }
        self.update_ambient(now);
    }

    /// Clears finished effects and resets zones whose idle time has run
    /// out (0 = never).
    pub fn tick(&mut self, now: Instant) {
        self.effects.retain(|e| now.saturating_duration_since(e.start).as_secs_f64() < e.secs);
        if !self.pending.is_empty() {
            let fade = Duration::from_millis(120);
            let (due, later): (Vec<_>, Vec<_>) = self.pending.drain(..).partition(|(t, _)| *t <= now);
            self.pending = later;
            for (_, i) in due {
                self.set_state(i, KeyState::Base, now, fade);
            }
        }
        if self.cfg.timeout_secs > 0 {
            let timeout = Duration::from_secs(self.cfg.timeout_secs as u64);
            let dur = Duration::from_millis(self.cfg.reset_fade_ms as u64);
            for z in 0..self.last_used.len() {
                if let Some(t) = self.last_used[z] {
                    if now.saturating_duration_since(t) >= timeout {
                        self.reset_zone(z, now, dur);
                    }
                }
            }
        }
        self.update_ambient(now);
    }

    fn fast_animating(&self, now: Instant) -> bool {
        self.fades.iter().any(|f| !f.done(now))
            || !self.pending.is_empty()
            || self.effects.iter().any(|e| now.saturating_duration_since(e.start).as_secs_f64() < e.secs)
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.fast_animating(now) || self.ambient_visible(now)
    }

    /// How often to redraw; `None` = everything is still.
    pub fn frame_interval(&self, now: Instant) -> Option<Duration> {
        if self.fast_animating(now) {
            Some(FRAME_FAST)
        } else if self.ambient_visible(now) {
            Some(FRAME_AMBIENT)
        } else {
            None
        }
    }

    /// Current colors per LED including the background and running effects,
    /// with brightness applied.
    pub fn colors(&self, now: Instant) -> Vec<[u8; 3]> {
        let b = self.cfg.brightness.min(100) as f32 / 100.0;
        let level = self.ambient.value(now) * self.cfg.ambient.intensity.min(100) as f32 / 100.0;
        let t = now.saturating_duration_since(self.clock).as_secs_f64() * self.cfg.ambient.speed.clamp(0.1, 5.0) as f64;
        let bounds = self.bounds();
        self.fades
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let mut c = f.value(now);
                if level > 0.0 && self.state[i] == KeyState::Base {
                    c = mix(c, self.ambient_color(i, t, bounds), level);
                }
                for e in &self.effects {
                    let t = now.saturating_duration_since(e.start).as_secs_f64();
                    if let Some((w, color)) = self.overlay(e, i, t) {
                        c = mix(c, color, w);
                    }
                }
                c.map(|v| (v * b).round().clamp(0.0, 255.0) as u8)
            })
            .collect()
    }
}

/// All LEDs up to `depth` steps away from `start`, only via allowed LEDs.
fn ring(start: usize, depth: usize, adj: &[Vec<usize>], allowed: impl Fn(usize) -> bool) -> Vec<usize> {
    let mut dist = vec![usize::MAX; adj.len()];
    dist[start] = 0;
    let mut queue = VecDeque::from([start]);
    let mut out = Vec::new();
    while let Some(i) = queue.pop_front() {
        if dist[i] == depth {
            continue;
        }
        for &j in &adj[i] {
            if dist[j] == usize::MAX && allowed(j) {
                dist[j] = dist[i] + 1;
                out.push(j);
                queue.push_back(j);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Without background effect, so color comparisons are exact.
    fn quiet() -> Config {
        Config { ambient: crate::config::Ambient { mode: AmbientMode::Off, ..Default::default() }, ..Config::default() }
    }

    fn engine() -> (Engine, Instant) {
        let now = Instant::now();
        (Engine::new(Layout::fallback(), quiet(), now), now)
    }

    fn state(e: &Engine, name: &str) -> KeyState {
        e.states()[e.layout().index_of(name).unwrap()]
    }

    fn colored(e: &Engine) -> Vec<String> {
        let mut v: Vec<String> = e
            .states()
            .iter()
            .enumerate()
            .filter(|(_, s)| **s != KeyState::Base)
            .map(|(i, _)| e.layout().leds[i].name.clone())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn first_press_colors_the_ring_primary() {
        let (mut e, now) = engine();
        e.press("D", now);
        assert_eq!(colored(&e), ["C", "E", "F", "R", "S", "X"]);
        assert_eq!(state(&e, "S"), KeyState::Primary);
        assert_eq!(state(&e, "D"), KeyState::Base);
    }

    #[test]
    fn pressing_a_colored_key_resets_it_and_rings_secondary() {
        let (mut e, now) = engine();
        e.press("D", now);
        e.press("F", now); // F is colored now (ring of D)
        assert_eq!(state(&e, "F"), KeyState::Base);
        for k in ["D", "G", "R", "T", "C", "V"] {
            assert_eq!(state(&e, k), KeyState::Secondary, "{k}");
        }
        // Rest of the old ring stays primary
        assert_eq!(state(&e, "S"), KeyState::Primary);
    }

    #[test]
    fn colored_neighbors_cycle_on() {
        let (mut e, now) = engine();
        e.press("U", now); // I gets ring 1
        assert_eq!(state(&e, "I"), KeyState::Primary);
        e.press("O", now); // I is a neighbor of O and moves on
        assert_eq!(state(&e, "I"), KeyState::Secondary);
        assert_eq!(state(&e, "P"), KeyState::Primary, "free neighbors get ring 1");
        e.press("O", now); // O stayed base color → I moves back again
        assert_eq!(state(&e, "I"), KeyState::Primary);
        assert_eq!(state(&e, "P"), KeyState::Secondary);
    }

    #[test]
    fn colors_never_get_stuck() {
        let (mut e, now) = engine();
        e.press("D", now); // S ring 1
        e.press("F", now); // F was ring 1 → D ring 2
        e.press("D", now); // D was ring 2 → free neighbors ring 1, colored ones move on
        assert_eq!(state(&e, "D"), KeyState::Base);
        assert_eq!(state(&e, "S"), KeyState::Secondary, "S was ring 1");
        assert_eq!(state(&e, "F"), KeyState::Primary, "F was base color");
        assert_eq!(state(&e, "R"), KeyState::Primary, "R was ring 2");
    }

    #[test]
    fn ring_stays_inside_its_zone() {
        let (mut e, now) = engine();
        e.press("Q", now);
        // Q borders 1 and 2 (number row), those stay off
        assert_eq!(state(&e, "1"), KeyState::Base);
        assert_eq!(state(&e, "2"), KeyState::Base);
        assert_eq!(state(&e, "W"), KeyState::Primary);
        assert_eq!(state(&e, "A"), KeyState::Primary);
        e.press("F4", now);
        assert_eq!(state(&e, "F5"), KeyState::Primary);
        assert_eq!(state(&e, "4"), KeyState::Base);
    }

    #[test]
    fn space_resets_letters_and_numbers_only() {
        let (mut e, now) = engine();
        e.press("D", now);
        e.press("5", now);
        e.press("F6", now);
        e.press("Space", now);
        assert_eq!(state(&e, "S"), KeyState::Base);
        assert_eq!(state(&e, "6"), KeyState::Base);
        assert_eq!(state(&e, "F7"), KeyState::Primary);
    }

    #[test]
    fn enter_resets_like_space() {
        let (mut e, now) = engine();
        e.press("D", now);
        e.press("5", now);
        e.press("Enter", now);
        assert_eq!(state(&e, "S"), KeyState::Base);
        assert_eq!(state(&e, "6"), KeyState::Base);
        assert_eq!(state(&e, "Enter"), KeyState::Base);
    }

    #[test]
    fn backspace_cycles_letters_and_numbers() {
        let (mut e, now) = engine();
        e.press("Backspace", now);
        for k in ["Q", "M", "5", "Backspace"] {
            assert_eq!(state(&e, k), KeyState::Primary, "{k}");
        }
        assert_eq!(state(&e, "F1"), KeyState::Base, "F-row stays");
        e.press("Backspace", now);
        assert_eq!(state(&e, "Q"), KeyState::Secondary);
        e.press("Backspace", now);
        assert_eq!(state(&e, "Q"), KeyState::Base);

        // A pattern moves on as a whole.
        e.press("D", now); // S primary, D base
        e.press("Backspace", now);
        assert_eq!(state(&e, "S"), KeyState::Secondary);
        assert_eq!(state(&e, "D"), KeyState::Primary);
    }

    #[test]
    fn effect_keys_do_not_ring_and_leave_colors_alone() {
        let (mut e, now) = engine();
        e.press("D", now);
        let later = now + Duration::from_secs(3);
        let (states, colors) = (e.states().to_vec(), e.colors(later));
        e.press("Tab", now);
        e.press("LeftAlt", now);
        e.press("LeftGui", now);
        e.press("CapsLock", now);
        assert_eq!(e.states(), &states[..], "effects don't change any states");
        assert!(e.animating(now + Duration::from_millis(300)));
        e.tick(later);
        assert!(!e.animating(later));
        assert_eq!(e.colors(later), colors, "after the effect the colors are as before");
    }

    #[test]
    fn cmd_backspace_wipes_letters_right_to_left() {
        let (mut e, now) = engine();
        e.press("Q", now);
        e.press("P", now);
        assert_eq!(state(&e, "W"), KeyState::Primary);
        assert_eq!(state(&e, "O"), KeyState::Primary);
        assert_eq!(e.press_combo("Backspace", combo::GUI, now).as_deref(), Some("Gui+Backspace"));
        assert_eq!(state(&e, "Backspace"), KeyState::Base, "combo colors no ring");
        e.tick(now + Duration::from_millis(230));
        assert_eq!(state(&e, "O"), KeyState::Base, "right side is already wiped");
        assert_eq!(state(&e, "W"), KeyState::Primary, "left side not yet");
        e.tick(now + Duration::from_millis(600));
        assert_eq!(state(&e, "W"), KeyState::Base);
    }

    #[test]
    fn alt_backspace_is_only_an_effect() {
        let (mut e, now) = engine();
        e.press("D", now);
        let states = e.states().to_vec();
        assert_eq!(e.press_combo("Backspace", combo::ALT, now).as_deref(), Some("Alt+Backspace"));
        e.tick(now + Duration::from_secs(1));
        assert_eq!(e.states(), &states[..]);
    }

    #[test]
    fn unknown_combo_falls_back_to_the_key() {
        let (mut e, now) = engine();
        assert_eq!(e.press_combo("H", combo::SHIFT, now).as_deref(), Some("H"), "capital letter = normal ring");
        assert_eq!(state(&e, "J"), KeyState::Primary);
    }

    fn color(e: &Engine, name: &str, at: Instant) -> [u8; 3] {
        e.colors(at)[e.layout().index_of(name).unwrap()]
    }

    #[test]
    fn background_plays_while_clean_and_freezes_on_a_ring() {
        let now = Instant::now();
        let mut e = Engine::new(Layout::fallback(), Config::default(), now);
        let base = parse_hex(&Config::default().palette.base);
        let s = |secs: f64| now + Duration::from_secs_f64(secs);
        assert_ne!(color(&e, "F1", s(3.0)), base, "background runs on a clean keyboard");
        assert_ne!(color(&e, "F1", s(3.0)), color(&e, "F1", s(6.0)), "and moves");
        assert_eq!(e.frame_interval(s(3.0)), Some(FRAME_AMBIENT));

        e.press("D", s(6.0));
        assert_eq!(color(&e, "F1", s(6.5)), base, "first ring makes the background static");
        e.tick(s(7.0));
        assert_eq!(e.frame_interval(s(7.0)), None);

        e.press("Space", s(8.0)); // letters and numbers reset → clean again
        assert_ne!(color(&e, "F1", s(10.0)), base, "after the reset it runs again");
    }

    #[test]
    fn every_background_mode_stays_in_palette_range() {
        use crate::config::Ambient;
        let now = Instant::now();
        for mode in [AmbientMode::Aurora, AmbientMode::Wave, AmbientMode::Fade, AmbientMode::Breathe, AmbientMode::Glimmer] {
            let cfg = Config { ambient: Ambient { mode, speed: 1.0, intensity: 100 }, ..Config::default() };
            let e = Engine::new(Layout::fallback(), cfg, now);
            let a = e.colors(now + Duration::from_secs(5));
            let b = e.colors(now + Duration::from_secs(9));
            assert_ne!(a, b, "{mode:?} moves");
        }
        let off = Engine::new(Layout::fallback(), quiet(), now);
        assert_eq!(off.frame_interval(now + Duration::from_secs(5)), None, "off = static, no redraw");
    }

    #[test]
    fn shift_tap_rises_from_the_bottom() {
        let (mut e, now) = engine();
        let base = parse_hex(&Config::default().palette.base);
        e.press("LeftShift", now);
        assert_eq!(state(&e, "CapsLock"), KeyState::Base, "a tap rings nothing, it only plays the effect");
        let early = now + Duration::from_millis(120);
        assert_ne!(color(&e, "LeftCtrl", early), base, "bottom row lights up first");
        assert_eq!(color(&e, "F5", early), base, "top row not yet");
        assert_ne!(color(&e, "F5", now + Duration::from_millis(500)), base, "then the stripe reaches the top");
        assert!(!e.animating(now + Duration::from_secs(1)));
    }

    #[test]
    fn wave_runs_outward_from_the_key() {
        let (mut e, now) = engine();
        let base = parse_hex(&Config::default().palette.base);
        let near = e.layout().index_of("LeftGui").unwrap();
        let far = e.layout().index_of("RightArrow").unwrap();
        e.press("LeftAlt", now);
        let early = e.colors(now + Duration::from_millis(60));
        assert_ne!(early[near], base, "wave has reached the neighboring key");
        assert_eq!(early[far], base, "but not yet the other end");
        let late = e.colors(now + Duration::from_millis(800));
        assert_ne!(late[far], base, "later it arrives on the right");
    }

    #[test]
    fn idle_zones_reset_after_timeout() {
        let (mut e, now) = engine();
        e.press("D", now);
        e.press("F6", now + Duration::from_secs(40));
        e.tick(now + Duration::from_secs(59));
        assert_eq!(state(&e, "S"), KeyState::Primary);
        e.tick(now + Duration::from_secs(61));
        assert_eq!(state(&e, "S"), KeyState::Base);
        assert_eq!(state(&e, "F7"), KeyState::Primary, "F-row was active only 21 s ago");
    }

    #[test]
    fn fades_land_on_palette_colors() {
        let (mut e, now) = engine();
        let i = e.layout().index_of("S").unwrap();
        assert_eq!(e.colors(now)[i], parse_hex(&Config::default().palette.base));
        e.press("D", now);
        let later = now + Duration::from_secs(1);
        assert!(!e.animating(later));
        assert_eq!(e.colors(later)[i], parse_hex(&Config::default().palette.primary));
    }

    #[test]
    fn reach_two_includes_next_ring() {
        let (mut e, now) = engine();
        let cfg = Config { reach: 2, ..quiet() };
        e.set_config(cfg, now);
        e.press("D", now);
        assert_eq!(state(&e, "G"), KeyState::Primary);
        assert_eq!(state(&e, "W"), KeyState::Primary);
    }

    #[test]
    fn iso_hash_key_resolves_from_backslash() {
        let (mut e, now) = engine();
        assert_eq!(e.press("Backslash", now).as_deref(), Some("NonUsTilde"));
    }

    #[test]
    fn swap_alt_gui() {
        let (mut e, now) = engine();
        e.set_config(Config { swap_alt_gui: true, ..quiet() }, now);
        assert_eq!(e.press("LeftAlt", now).as_deref(), Some("LeftGui"));
    }

    #[test]
    fn own_zone_colors() {
        let (mut e, now) = engine();
        let mut cfg = quiet();
        let frow = cfg.zones.iter_mut().find(|z| z.id == "frow").unwrap();
        frow.own_colors = true;
        frow.palette.base = "#00E5C8".into();
        e.set_config(cfg, now);
        let i = e.layout().index_of("F1").unwrap();
        assert_eq!(e.colors(now + Duration::from_secs(1))[i], [0x00, 0xE5, 0xC8]);
    }
}
