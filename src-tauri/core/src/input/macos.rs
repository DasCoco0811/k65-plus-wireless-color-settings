//! macOS: CGEventTap in listen-only mode on its own run loop thread.

use super::{InputStatus, Key, KeySink, SharedStatus};
use crate::combo::ComboTracker;
use crate::keymap::{is_character_key, mac_keycode, mac_media_key, mac_special_keycode};
use std::sync::Mutex;
use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};
use std::time::Duration;

type CGEventRef = *mut c_void;
type CFMachPortRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type TapCallback = extern "C" fn(*mut c_void, u32, CGEventRef, *mut c_void) -> CGEventRef;

const SESSION_EVENT_TAP: u32 = 1;
const HID_EVENT_TAP: u32 = 0;
const HEAD_INSERT: u32 = 0;
const LISTEN_ONLY: u32 = 1;
const KEY_DOWN: u32 = 10;
const FLAGS_CHANGED: u32 = 12;
/// `NSEventTypeSystemDefined`: media and special keys (volume, brightness, …).
const SYSTEM_DEFINED: u32 = 14;
/// Subtype of system events for media/special keys (`NX_SUBTYPE_AUX_CONTROL_BUTTONS`).
const SUBTYPE_AUX_CONTROL_BUTTONS: i16 = 8;
const TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
const FIELD_AUTOREPEAT: u32 = 8;
const FIELD_KEYCODE: u32 = 9;
const FIELD_KEYBOARD_TYPE: u32 = 10;
const REQUEST_LISTEN_EVENT: u32 = 1;
const ACCESS_GRANTED: u32 = 0;
const ACCESS_DENIED: u32 = 1;
/// `kKeyboardISO` from HIToolbox (`'ISO '`).
const KEYBOARD_ISO: u32 = 0x4953_4F20;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: TapCallback,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CGEventCreateKeyboardEvent(source: *const c_void, keycode: u16, down: bool) -> CGEventRef;
    fn CGEventSetType(event: CGEventRef, kind: u32);
    fn CGEventSetFlags(event: CGEventRef, flags: u64);
    fn CGEventPost(tap: u32, event: CGEventRef);
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFMachPortCreateRunLoopSource(alloc: *const c_void, port: CFMachPortRef, order: isize) -> CFRunLoopSourceRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: *const c_void);
    fn CFRunLoopRun();
    fn CFRunLoopRunInMode(mode: *const c_void, seconds: f64, return_after_source_handled: bool) -> i32;
    fn CFRelease(obj: *const c_void);
    static kCFRunLoopCommonModes: *const c_void;
    static kCFRunLoopDefaultMode: *const c_void;
}

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOHIDCheckAccess(request: u32) -> u32;
    fn IOHIDRequestAccess(request: u32) -> bool;
}

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn KBGetLayoutType(keyboard_type: i16) -> u32;
}

// The data of media keys (`data1`) is only available via NSEvent.
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    fn objc_msgSend();
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

/// Media/special key from a system event: `NX_KEYTYPE_*`, pressed?, repeat?
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MediaKey {
    pub key: u32,
    pub down: bool,
    pub repeat: bool,
}

fn media_key(event: CGEventRef) -> Option<MediaKey> {
    type NewEvent = unsafe extern "C" fn(*mut c_void, *mut c_void, CGEventRef) -> *mut c_void;
    type GetI16 = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i16;
    type GetIsize = unsafe extern "C" fn(*mut c_void, *mut c_void) -> isize;
    unsafe {
        let send = objc_msgSend as *const ();
        let pool = objc_autoreleasePoolPush();
        let class = objc_getClass(c"NSEvent".as_ptr());
        let ns_event = std::mem::transmute::<*const (), NewEvent>(send)(class, sel_registerName(c"eventWithCGEvent:".as_ptr()), event);
        let result = if ns_event.is_null() {
            None
        } else {
            let subtype = std::mem::transmute::<*const (), GetI16>(send)(ns_event, sel_registerName(c"subtype".as_ptr()));
            let data1 = std::mem::transmute::<*const (), GetIsize>(send)(ns_event, sel_registerName(c"data1".as_ptr()));
            (subtype == SUBTYPE_AUX_CONTROL_BUTTONS).then(|| MediaKey {
                key: ((data1 as u64 & 0xFFFF_0000) >> 16) as u32,
                down: (data1 & 0xFF00) >> 8 == 0x0A,
                repeat: data1 & 0x1 != 0,
            })
        };
        objc_autoreleasePoolPop(pool);
        result
    }
}

static TAP: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
/// Cache for the last seen keyboard type and whether it is ISO.
static LAST_KBD_TYPE: AtomicU32 = AtomicU32::new(u32::MAX);
static LAST_KBD_ISO: AtomicU32 = AtomicU32::new(0);

pub fn permission() -> Option<bool> {
    match unsafe { IOHIDCheckAccess(REQUEST_LISTEN_EVENT) } {
        ACCESS_GRANTED => Some(true),
        ACCESS_DENIED => Some(false),
        _ => None,
    }
}

pub fn request_permission() -> bool {
    unsafe { IOHIDRequestAccess(REQUEST_LISTEN_EVENT) }
}

fn is_iso(kbd_type: i64) -> bool {
    let t = kbd_type as u32;
    if LAST_KBD_TYPE.load(Ordering::Relaxed) != t {
        let iso = unsafe { KBGetLayoutType(kbd_type as i16) } == KEYBOARD_ISO;
        LAST_KBD_ISO.store(iso as u32, Ordering::Relaxed);
        LAST_KBD_TYPE.store(t, Ordering::Relaxed);
    }
    LAST_KBD_ISO.load(Ordering::Relaxed) == 1
}

/// Whether a modifier change was a press (`true`) or a release:
/// device-dependent flag bits (left/right separate) from IOLLEvent.h.
/// Caps Lock always counts as a press.
fn modifier_down(code: u16, flags: u64) -> Option<bool> {
    let bit: u64 = match code {
        0x38 => 0x0000_0002, // left Shift
        0x3C => 0x0000_0004, // right Shift
        0x3B => 0x0000_0001, // left Ctrl
        0x3E => 0x0000_2000, // right Ctrl
        0x3A => 0x0000_0020, // left Option
        0x3D => 0x0000_0040, // right Option
        0x37 => 0x0000_0008, // left Cmd
        0x36 => 0x0000_0010, // right Cmd
        0x3F => 0x0080_0000, // Fn
        0x39 => return Some(true), // Caps Lock
        _ => return None,
    };
    Some(flags & bit != 0)
}

/// What the tap callback receives via `user_info`.
struct TapContext {
    sink: KeySink,
    tracker: Mutex<ComboTracker>,
}

extern "C" fn on_event(_proxy: *mut c_void, kind: u32, event: CGEventRef, user: *mut c_void) -> CGEventRef {
    match kind {
        TAP_DISABLED_BY_TIMEOUT | TAP_DISABLED_BY_USER_INPUT => {
            let tap = TAP.load(Ordering::Relaxed);
            if !tap.is_null() {
                unsafe { CGEventTapEnable(tap, true) };
            }
        }
        KEY_DOWN | FLAGS_CHANGED => {
            let ctx = unsafe { &*(user as *const TapContext) };
            let field = |f| unsafe { CGEventGetIntegerValueField(event, f) };
            let code = field(FIELD_KEYCODE) as u16;
            let name = mac_keycode(code, is_iso(field(FIELD_KEYBOARD_TYPE)));
            let mut tracker = ctx.tracker.lock().unwrap();
            let out = if kind == KEY_DOWN {
                (field(FIELD_AUTOREPEAT) == 0).then(|| match name {
                    Some(n) => Key::Led(n),
                    None => Key::Other(mac_special_keycode(code)),
                })
                .and_then(|key| tracker.down(key))
            } else {
                match (modifier_down(code, unsafe { CGEventGetFlags(event) }), name) {
                    (Some(true), Some(n)) => tracker.down(Key::Led(n)),
                    (Some(false), Some(n)) => tracker.up(n),
                    _ => None,
                }
            };
            drop(tracker);
            if let Some(input) = out {
                (ctx.sink)(input);
            }
        }
        SYSTEM_DEFINED => {
            let token = media_key(event).filter(|m| m.down && !m.repeat).and_then(|m| mac_media_key(m.key));
            if let Some(token) = token {
                let ctx = unsafe { &*(user as *const TapContext) };
                let out = ctx.tracker.lock().unwrap().down(Key::Other(token));
                if let Some(input) = out {
                    (ctx.sink)(input);
                }
            }
        }
        _ => {}
    }
    event
}

pub fn start(sink: KeySink, status: SharedStatus) {
    std::thread::Builder::new()
        .name("k65-input".into())
        .spawn(move || {
            // On the very first start this makes macOS show the permission dialog.
            if permission().is_none() {
                request_permission();
            }
            let ctx = TapContext { sink, tracker: Mutex::new(ComboTracker::new()) };
            let user = Box::into_raw(Box::new(ctx)) as *mut c_void;
            let mask = (1u64 << KEY_DOWN) | (1u64 << FLAGS_CHANGED) | (1u64 << SYSTEM_DEFINED);
            loop {
                let tap = unsafe { CGEventTapCreate(SESSION_EVENT_TAP, HEAD_INSERT, LISTEN_ONLY, mask, on_event, user) };
                if tap.is_null() {
                    *status.lock().unwrap() = match permission() {
                        Some(true) => InputStatus::Failed("could not create the event tap".into()),
                        _ => InputStatus::NoPermission,
                    };
                    std::thread::sleep(Duration::from_secs(2));
                    continue;
                }
                TAP.store(tap, Ordering::Relaxed);
                unsafe {
                    let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
                    CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
                    CFRelease(source);
                    CGEventTapEnable(tap, true);
                }
                *status.lock().unwrap() = InputStatus::Running;
                unsafe { CFRunLoopRun() };
                // The run loop only ends when the source goes away: create it again.
                TAP.store(std::ptr::null_mut(), Ordering::Relaxed);
                *status.lock().unwrap() = InputStatus::Starting;
                std::thread::sleep(Duration::from_secs(1));
            }
        })
        .expect("input thread");
}

/// Posts a synthetic press and release of the left Shift key at HID level
/// (for self-tests; Shift alone types nothing). Needs the Accessibility
/// permission of the calling program.
pub fn post_test_shift() {
    const LEFT_SHIFT: u16 = 0x38;
    const SHIFT_FLAGS: u64 = 0x0002_0000 | 0x0000_0002;
    unsafe {
        for (down, flags) in [(true, SHIFT_FLAGS), (false, 0)] {
            let ev = CGEventCreateKeyboardEvent(std::ptr::null(), LEFT_SHIFT, down);
            if ev.is_null() {
                continue;
            }
            CGEventSetType(ev, FLAGS_CHANGED);
            CGEventSetFlags(ev, flags);
            CGEventPost(HID_EVENT_TAP, ev);
            CFRelease(ev);
        }
    }
}

type DiagSink = Box<dyn Fn(String)>;

extern "C" fn on_diag_event(_proxy: *mut c_void, kind: u32, event: CGEventRef, user: *mut c_void) -> CGEventRef {
    let sink = unsafe { &*(user as *const DiagSink) };
    let field = |f| unsafe { CGEventGetIntegerValueField(event, f) };
    let line = match kind {
        KEY_DOWN => {
            let code = field(FIELD_KEYCODE) as u16;
            let name = mac_keycode(code, is_iso(field(FIELD_KEYBOARD_TYPE)));
            if name.is_some_and(is_character_key) {
                return event; // Text is never recorded.
            }
            format!("key        code={code:#04x} → {:?}{}", name, if field(FIELD_AUTOREPEAT) != 0 { " (repeat)" } else { "" })
        }
        FLAGS_CHANGED => {
            let code = field(FIELD_KEYCODE) as u16;
            format!("Modifier   code={code:#04x} flags={:#010x}", unsafe { CGEventGetFlags(event) })
        }
        SYSTEM_DEFINED => match media_key(event) {
            Some(m) => format!("media      NX_KEYTYPE={} {}{}", m.key, if m.down { "down" } else { "up" }, if m.repeat { " (repeat)" } else { "" }),
            None => "system     (other subtype)".to_string(),
        },
        _ => return event,
    };
    sink(line);
    event
}

/// For `duration`, writes every special, modifier and media event as a line
/// of text to `sink` (to find out what a keyboard really sends).
/// Letters, digits and punctuation are deliberately skipped.
pub fn diagnose(duration: Duration, sink: DiagSink) -> Result<(), String> {
    let user = Box::into_raw(Box::new(sink)) as *mut c_void;
    let mask = (1u64 << KEY_DOWN) | (1u64 << FLAGS_CHANGED) | (1u64 << SYSTEM_DEFINED);
    let tap = unsafe { CGEventTapCreate(SESSION_EVENT_TAP, HEAD_INSERT, LISTEN_ONLY, mask, on_diag_event, user) };
    if tap.is_null() {
        return Err("could not create the event tap (Input Monitoring permission?)".into());
    }
    unsafe {
        let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopDefaultMode);
        CGEventTapEnable(tap, true);
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, duration.as_secs_f64(), false);
        CGEventTapEnable(tap, false);
        CFRelease(source);
        CFRelease(tap);
    }
    Ok(())
}
