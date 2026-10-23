//! System-wide listening for key presses (only presses, no content).
//!
//! The iCUE SDK only reports macro keys, so we listen ourselves: macOS via an
//! event tap (needs "Input Monitoring"), Windows via a low-level keyboard
//! hook. Only which key it was gets passed on.

use serde::Serialize;
use std::sync::{Arc, Mutex};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[derive(Clone, Debug, PartialEq)]
pub enum Key {
    /// Key with its own LED (position-based LED name).
    Led(&'static str),
    /// Media or special key without a fixed LED, e.g. `"VolumeUp"` or
    /// `"mac:0xa0"`. The app learns from the user which LED belongs to it.
    Other(String),
}

/// A triggered key press including held modifiers (bits from [`crate::combo`]).
#[derive(Clone, Debug, PartialEq)]
pub struct KeyInput {
    pub key: Key,
    pub mods: u8,
}

pub type KeySink = Arc<dyn Fn(KeyInput) + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", content = "detail", rename_all = "snake_case")]
pub enum InputStatus {
    Starting,
    Running,
    NoPermission,
    Failed(String),
    Unsupported,
}

pub type SharedStatus = Arc<Mutex<InputStatus>>;

/// Starts the listener on its own thread. If the permission is missing, it
/// retries every few seconds.
pub fn start(sink: KeySink, status: SharedStatus) {
    #[cfg(target_os = "macos")]
    macos::start(sink, status);
    #[cfg(windows)]
    windows::start(sink, status);
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = sink;
        *status.lock().unwrap() = InputStatus::Unsupported;
    }
}

/// `Some(true)` = granted, `Some(false)` = denied, `None` = unknown.
pub fn permission() -> Option<bool> {
    #[cfg(target_os = "macos")]
    return macos::permission();
    #[cfg(not(target_os = "macos"))]
    return Some(true);
}

/// Requests the permission (macOS then shows the system dialog).
pub fn request_permission() -> bool {
    #[cfg(target_os = "macos")]
    return macos::request_permission();
    #[cfg(not(target_os = "macos"))]
    return true;
}

#[cfg(target_os = "macos")]
pub use macos::{diagnose, post_test_shift};
