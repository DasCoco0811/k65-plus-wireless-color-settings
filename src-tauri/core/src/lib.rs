//! Core of K65 Plus Wireless Color Settings: iCUE SDK binding, keyboard layout,
//! light logic and system-wide keyboard input. Deliberately contains nothing
//! from Tauri, so the core can also be checked and tested for Windows.

pub mod combo;
pub mod config;
pub mod engine;
pub mod input;
pub mod keymap;
pub mod layout;
pub mod sdk;
pub mod service;
