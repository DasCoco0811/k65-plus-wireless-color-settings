//! Windows: WH_KEYBOARD_LL hook on its own thread with a message loop.

use super::{InputStatus, Key, KeySink, SharedStatus};
use crate::combo::ComboTracker;
use crate::keymap::{win_other_key, win_scancode};
use std::sync::{Mutex, OnceLock};
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage, HC_ACTION,
    KBDLLHOOKSTRUCT, LLKHF_EXTENDED, LLKHF_INJECTED, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
    WM_SYSKEYDOWN, WM_SYSKEYUP,
};

static SINK: OnceLock<KeySink> = OnceLock::new();
/// Pressed keys (scan code + 0x100 for extended ones), to filter out repeats.
static DOWN: Mutex<[bool; 0x200]> = Mutex::new([false; 0x200]);
static TRACKER: Mutex<ComboTracker> = Mutex::new(ComboTracker::new());

/// On German layouts AltGr also produces a synthetic left Ctrl.
const ALTGR_FAKE_CTRL: u32 = 0x21D;

unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let info = &*(lparam as *const KBDLLHOOKSTRUCT);
        let injected = info.flags & LLKHF_INJECTED != 0;
        if !injected && info.scanCode != ALTGR_FAKE_CTRL {
            let ext = info.flags & LLKHF_EXTENDED != 0;
            let slot = ((info.scanCode & 0xFF) | if ext { 0x100 } else { 0 }) as usize;
            let msg = wparam as u32;
            let scan = info.scanCode & 0xFF;
            let out = if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                let was_down = std::mem::replace(&mut DOWN.lock().unwrap()[slot], true);
                let key = match win_scancode(scan, ext) {
                    Some(name) => Some(Key::Led(name)),
                    None if scan != 0 => Some(Key::Other(win_other_key(scan, ext))),
                    None => None,
                };
                key.filter(|_| !was_down).and_then(|k| TRACKER.lock().unwrap().down(k))
            } else if msg == WM_KEYUP || msg == WM_SYSKEYUP {
                DOWN.lock().unwrap()[slot] = false;
                win_scancode(scan, ext).and_then(|name| TRACKER.lock().unwrap().up(name))
            } else {
                None
            };
            if let (Some(input), Some(sink)) = (out, SINK.get()) {
                sink(input);
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

pub fn start(sink: KeySink, status: SharedStatus) {
    let _ = SINK.set(sink);
    std::thread::Builder::new()
        .name("k65-input".into())
        .spawn(move || unsafe {
            let module = GetModuleHandleW(std::ptr::null());
            let hhook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), module, 0);
            if hhook.is_null() {
                *status.lock().unwrap() = InputStatus::Failed("could not install the keyboard hook".into());
                return;
            }
            *status.lock().unwrap() = InputStatus::Running;
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        })
        .expect("input thread");
}
