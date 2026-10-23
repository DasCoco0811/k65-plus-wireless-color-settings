//! Runtime: a light thread ties together iCUE, the light logic and keyboard input.
//! The UI talks to it only through [`Service`].

use crate::config::{parse_hex, to_led, Config};
use crate::engine::Engine;
use crate::input::{self, InputStatus, Key, KeyInput, SharedStatus};
use crate::layout::Layout;
use crate::sdk::{self, Sdk, SdkEvent};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Above iCUE (127) and the default for SDK programs (128).
const LAYER_PRIORITY: u32 = 200;
const IDLE: Duration = Duration::from_millis(250);
const RESCAN_EVERY: Duration = Duration::from_secs(2);
/// Resend colors regularly in case iCUE has reinitialized the device.
const KEEPALIVE: Duration = Duration::from_secs(3);
const BATTERY_EVERY: Duration = Duration::from_secs(60);
const PERMISSION_EVERY: Duration = Duration::from_secs(2);
const PALETTE_PREVIEW: Duration = Duration::from_secs(6);

pub enum Command {
    SetConfig(Config),
    /// Key press from the UI, optionally with modifier bits.
    Press { name: String, mods: u8 },
    ResetAll,
    /// Briefly shows the base color, ring 1 and ring 2 as stripes on the keyboard.
    ShowPalette,
    Shutdown,
}

enum Msg {
    Key(KeyInput),
    Cmd(Command),
    Sdk(SdkEvent),
}

#[derive(Clone, Debug, Serialize)]
pub struct DeviceView {
    pub model: String,
    pub led_count: i32,
    pub battery: Option<i32>,
    /// Logical key layout such as `"de"` or `"us"` (for keycap labels).
    pub layout: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LedView {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub zone: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    /// `loading_sdk` | `missing_sdk` | `connecting` | `connected` | `unavailable` | `refused`
    pub icue: &'static str,
    pub icue_version: Option<String>,
    pub sdk_error: Option<String>,
    pub device: Option<DeviceView>,
    pub input: InputStatus,
    pub input_permission: Option<bool>,
    pub leds: Vec<LedView>,
    pub colors: Vec<[u8; 3]>,
    pub last_key: Option<String>,
    /// Last pressed media/special key that is not yet assigned to a key.
    pub unknown_key: Option<String>,
    pub paused: bool,
}

pub struct Service {
    tx: Sender<Msg>,
    shared: Arc<Mutex<Snapshot>>,
    input_status: SharedStatus,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Service {
    pub fn start(cfg: Config, sdk_candidates: Vec<PathBuf>) -> Service {
        let (tx, rx) = mpsc::channel();
        let input_status: SharedStatus = Arc::new(Mutex::new(InputStatus::Starting));
        let key_tx = tx.clone();
        input::start(Arc::new(move |name| drop(key_tx.send(Msg::Key(name)))), input_status.clone());

        let now = Instant::now();
        let worker = Worker {
            tx: tx.clone(),
            engine: Engine::new(Layout::fallback(), cfg, now),
            icue: "loading_sdk",
            sdk: None,
            sdk_candidates,
            sdk_error: None,
            icue_version: None,
            device: None,
            last_scan: None,
            last_push: None,
            pushed: Vec::new(),
            paused_pushed: false,
            palette_preview_until: None,
            last_key: None,
            unknown_key: None,
            permission: input::permission(),
            permission_at: now,
        };
        let shared = Arc::new(Mutex::new(worker.snapshot(now)));
        let thread = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("k65-light".into())
                .spawn(move || worker.run(rx, shared))
                .expect("light thread")
        };
        Service { tx, shared, input_status, thread: Mutex::new(Some(thread)) }
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut s = self.shared.lock().unwrap().clone();
        s.input = self.input_status.lock().unwrap().clone();
        s
    }

    pub fn send(&self, cmd: Command) {
        let _ = self.tx.send(Msg::Cmd(cmd));
    }

    /// Disconnects from iCUE (its lighting returns) and waits for the thread.
    pub fn shutdown(&self) {
        let _ = self.tx.send(Msg::Cmd(Command::Shutdown));
        if let Some(t) = self.thread.lock().unwrap().take() {
            let _ = t.join();
        }
    }
}

struct Active {
    id: String,
    model: String,
    led_count: i32,
    layout: Option<&'static str>,
    luids: Vec<u32>,
    battery: Option<i32>,
    battery_at: Instant,
}

struct Worker {
    tx: Sender<Msg>,
    sdk: Option<Sdk>,
    sdk_candidates: Vec<PathBuf>,
    sdk_error: Option<String>,
    engine: Engine,
    icue: &'static str,
    icue_version: Option<String>,
    device: Option<Active>,
    last_scan: Option<Instant>,
    last_push: Option<Instant>,
    pushed: Vec<[u8; 3]>,
    paused_pushed: bool,
    palette_preview_until: Option<Instant>,
    last_key: Option<String>,
    unknown_key: Option<String>,
    permission: Option<bool>,
    permission_at: Instant,
}

fn due(last: Option<Instant>, every: Duration, now: Instant) -> bool {
    last.map_or(true, |t| now.saturating_duration_since(t) >= every)
}

impl Worker {
    fn run(mut self, rx: Receiver<Msg>, shared: Arc<Mutex<Snapshot>>) {
        self.load_sdk(&shared);
        if let Some(sdk) = &self.sdk {
            let tx = self.tx.clone();
            if let Err(e) = sdk.connect(Box::new(move |ev| drop(tx.send(Msg::Sdk(ev))))) {
                self.sdk_error = Some(format!("connecting to iCUE: {}", sdk::error_text(e)));
            }
        }
        loop {
            let wait = self.engine.frame_interval(Instant::now()).unwrap_or(IDLE);
            let mut msgs = Vec::new();
            match rx.recv_timeout(wait) {
                Ok(m) => msgs.push(m),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            msgs.extend(rx.try_iter());
            if !msgs.into_iter().all(|m| self.handle(m)) {
                break;
            }
            let now = Instant::now();
            self.step(now);
            *shared.lock().unwrap() = self.snapshot(now);
        }
        if let Some(sdk) = &self.sdk {
            sdk.disconnect();
        }
    }

    /// Looks for the SDK and downloads it from Corsair on first start.
    fn load_sdk(&mut self, shared: &Arc<Mutex<Snapshot>>) {
        *shared.lock().unwrap() = self.snapshot(Instant::now());
        let loaded = Sdk::load_or_download(&self.sdk_candidates);
        match loaded {
            Ok(sdk) => {
                log::info!("iCUE SDK loaded: {}", sdk.path.display());
                self.sdk = Some(sdk);
                self.icue = "connecting";
            }
            Err(e) => {
                log::error!("iCUE SDK not loaded: {e}");
                self.sdk_error = Some(e);
                self.icue = "missing_sdk";
            }
        }
    }

    /// `false` ends the thread.
    fn handle(&mut self, msg: Msg) -> bool {
        let now = Instant::now();
        match msg {
            Msg::Key(KeyInput { key: Key::Led(name), mods }) => self.press(name, mods, now),
            Msg::Key(KeyInput { key: Key::Other(token), mods }) => match self.engine.config().learned_keys.get(&token).cloned() {
                Some(led) if !led.is_empty() => self.press(&led, mods, now),
                Some(_) => self.last_key = Some(token),
                None => {
                    self.last_key = Some(token.clone());
                    self.unknown_key = Some(token);
                }
            },
            Msg::Cmd(Command::Press { name, mods }) => self.press(&name, mods, now),
            Msg::Cmd(Command::SetConfig(cfg)) => {
                // Resend, even if only pause or color correction changed.
                self.paused_pushed = false;
                self.pushed.clear();
                if self.unknown_key.as_ref().is_some_and(|k| cfg.learned_keys.contains_key(k)) {
                    self.unknown_key = None;
                }
                self.engine.set_config(cfg, now);
            }
            Msg::Cmd(Command::ResetAll) => self.engine.reset_all(now),
            Msg::Cmd(Command::ShowPalette) => self.palette_preview_until = Some(now + PALETTE_PREVIEW),
            Msg::Cmd(Command::Shutdown) => return false,
            Msg::Sdk(SdkEvent::Session { state, icue_version }) => {
                if icue_version.is_some() {
                    self.icue_version = icue_version;
                }
                self.icue = match state {
                    sdk::CSS_CONNECTED => "connected",
                    3 | 5 => "unavailable",
                    4 => "refused",
                    _ => "connecting",
                };
                if state == sdk::CSS_CONNECTED {
                    self.on_connected();
                } else {
                    self.device = None;
                }
            }
            Msg::Sdk(SdkEvent::DevicesChanged) => self.last_scan = None,
        }
        true
    }

    fn on_connected(&mut self) {
        let Some(sdk) = &self.sdk else { return };
        if let Err(e) = sdk.set_layer_priority(LAYER_PRIORITY) {
            log::warn!("Layer priority: {}", sdk::error_text(e));
        }
        let tx = self.tx.clone();
        if let Err(e) = sdk.subscribe(Box::new(move |ev| drop(tx.send(Msg::Sdk(ev))))) {
            log::warn!("Device events: {}", sdk::error_text(e));
        }
        self.last_scan = None;
    }

    fn press(&mut self, name: &str, mods: u8, now: Instant) {
        self.last_key = Some(match self.engine.press_combo(name, mods, now) {
            Some(resolved) => resolved,
            None => format!("{name} (not on this keyboard)"),
        });
    }

    fn step(&mut self, now: Instant) {
        self.engine.tick(now);
        if self.icue == "connected" && due(self.last_scan, RESCAN_EVERY, now) {
            self.scan(now);
        }
        if now.saturating_duration_since(self.permission_at) >= PERMISSION_EVERY {
            self.permission = input::permission();
            self.permission_at = now;
        }
        self.push(now);
    }

    /// Looks for the keyboard. If the active keyboard stays connected, nothing changes.
    fn scan(&mut self, now: Instant) {
        let Some(sdk) = &self.sdk else { return };
        self.last_scan = Some(now);
        let keyboards = match sdk.keyboards() {
            Ok(k) => k,
            Err(e) => {
                log::warn!("Device list: {}", sdk::error_text(e));
                return;
            }
        };
        if let Some(dev) = &mut self.device {
            if keyboards.iter().any(|k| k.id == dev.id) {
                if due(Some(dev.battery_at), BATTERY_EVERY, now) {
                    dev.battery = sdk.battery(&dev.id);
                    dev.battery_at = now;
                }
                return;
            }
        }
        self.device = None;
        let Some(kb) = keyboards.into_iter().next() else { return };
        match sdk.led_positions(&kb.id) {
            Ok(positions) if !positions.is_empty() => {
                let layout = Layout::from_positions(&positions);
                log::info!("Keyboard detected: {} ({} LEDs)", kb.model, layout.leds.len());
                self.device = Some(Active {
                    luids: layout.leds.iter().map(|l| l.luid).collect(),
                    battery: sdk.battery(&kb.id),
                    battery_at: now,
                    layout: sdk.logical_layout(&kb.id),
                    id: kb.id,
                    model: kb.model,
                    led_count: kb.led_count,
                });
                self.engine.set_layout(layout, now);
                self.pushed.clear();
                self.paused_pushed = false;
            }
            Ok(_) => log::warn!("{} reports no LEDs", kb.model),
            Err(e) => log::warn!("LED positions: {}", sdk::error_text(e)),
        }
    }

    fn push(&mut self, now: Instant) {
        let (Some(sdk), Some(dev)) = (&self.sdk, &self.device) else { return };
        let keepalive = due(self.last_push, KEEPALIVE, now);
        let result = if self.engine.config().paused {
            if self.paused_pushed && !keepalive {
                return;
            }
            // Transparent layer: iCUE lighting shows through.
            let leds: Vec<(u32, [u8; 3])> = dev.luids.iter().map(|&l| (l, [0, 0, 0])).collect();
            self.paused_pushed = true;
            sdk.set_colors(&dev.id, &leds, 0)
        } else {
            let colors = self.display_colors(now);
            if colors == self.pushed && !keepalive {
                return;
            }
            let correct = self.engine.config().led_correction;
            let leds: Vec<(u32, [u8; 3])> = dev
                .luids
                .iter()
                .zip(&colors)
                .map(|(&luid, &c)| (luid, if correct { to_led(c) } else { c }))
                .collect();
            self.pushed = colors;
            sdk.set_colors(&dev.id, &leds, 255)
        };
        self.last_push = Some(now);
        if let Err(e) = result {
            log::warn!("Sending colors: {}", sdk::error_text(e));
            if e == sdk::CE_DEVICE_NOT_FOUND || e == sdk::CE_NOT_CONNECTED {
                self.device = None;
                self.last_scan = None;
            }
        }
    }

    /// Colors as screen values: normally from the light logic, during the palette
    /// preview three stripes from left to right (base color, ring 1, ring 2).
    fn display_colors(&self, now: Instant) -> Vec<[u8; 3]> {
        if !self.palette_preview_until.is_some_and(|t| now < t) {
            return self.engine.colors(now);
        }
        let cfg = self.engine.config();
        let b = cfg.brightness.min(100) as f32 / 100.0;
        let bands = [&cfg.palette.base, &cfg.palette.primary, &cfg.palette.secondary]
            .map(|h| parse_hex(h).map(|c| (c as f32 * b).round() as u8));
        let leds = &self.engine.layout().leds;
        let (x0, x1) = leds.iter().fold((f64::MAX, f64::MIN), |(a, z), l| (a.min(l.x), z.max(l.x)));
        let third = ((x1 - x0) / 3.0).max(1.0);
        leds.iter().map(|l| bands[(((l.x - x0) / third) as usize).min(2)]).collect()
    }

    fn snapshot(&self, now: Instant) -> Snapshot {
        let layout = self.engine.layout();
        let zones = self.engine.zone_of();
        Snapshot {
            icue: self.icue,
            icue_version: self.icue_version.clone(),
            sdk_error: self.sdk_error.clone(),
            device: self.device.as_ref().map(|d| DeviceView {
                model: d.model.clone(),
                led_count: d.led_count,
                battery: d.battery,
                layout: d.layout,
            }),
            input: InputStatus::Starting,
            input_permission: self.permission,
            leds: layout
                .leds
                .iter()
                .zip(zones)
                .map(|(l, z)| LedView { name: l.name.clone(), x: l.x, y: l.y, w: l.w, h: l.h, zone: *z })
                .collect(),
            colors: self.display_colors(now),
            last_key: self.last_key.clone(),
            unknown_key: self.unknown_key.clone(),
            paused: self.engine.config().paused,
        }
    }
}
