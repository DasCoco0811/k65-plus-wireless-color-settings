//! Thin binding to the iCUE SDK v4 (`iCUESDK.h`), loaded at runtime.
//!
//! iCUE itself must be running; the SDK connects to the iCUE service and
//! puts our colors as a separate layer on top of the iCUE lighting. On
//! disconnect the layer disappears and iCUE takes over again.

use libloading::Library;
use std::ffi::{c_char, c_int, c_uint, c_void};
use std::path::{Path, PathBuf};

const STRING_SIZE_M: usize = 128;
const DEVICE_COUNT_MAX: usize = 64;
const LEDCOUNT_MAX: usize = 512;

pub const CE_SUCCESS: c_int = 0;
pub const CE_NOT_CONNECTED: c_int = 1;
pub const CE_DEVICE_NOT_FOUND: c_int = 6;
pub const CSS_CONNECTED: c_int = 6;
const CDT_KEYBOARD: c_int = 0x0001;
const CDPI_LOGICAL_LAYOUT: c_int = 7;
const CDPI_BATTERY_LEVEL: c_int = 9;
const CEI_DEVICE_CONNECTION_STATUS_CHANGED: c_int = 1;

type DeviceId = [c_char; STRING_SIZE_M];

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CorsairVersion {
    major: c_int,
    minor: c_int,
    patch: c_int,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CorsairSessionDetails {
    client: CorsairVersion,
    server: CorsairVersion,
    host: CorsairVersion,
}

#[repr(C)]
struct CorsairSessionStateChanged {
    state: c_int,
    details: CorsairSessionDetails,
}

#[repr(C)]
struct CorsairDeviceInfo {
    kind: c_int,
    id: DeviceId,
    serial: [c_char; STRING_SIZE_M],
    model: [c_char; STRING_SIZE_M],
    led_count: c_int,
    channel_count: c_int,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CorsairLedPosition {
    id: c_uint,
    cx: f64,
    cy: f64,
}

#[repr(C)]
struct CorsairDeviceFilter {
    device_type_mask: c_int,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CorsairLedColor {
    id: c_uint,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

#[repr(C)]
struct CorsairEvent {
    id: c_int,
    data: *const c_void,
}

/// `CorsairProperty`: type plus 16-byte union (largest member: pointer + count).
#[repr(C)]
struct CorsairProperty {
    kind: c_int,
    value: [u64; 2],
}

type SessionCb = extern "C" fn(*mut c_void, *const CorsairSessionStateChanged);
type EventCb = extern "C" fn(*mut c_void, *const CorsairEvent);

struct Fns {
    connect: unsafe extern "C" fn(SessionCb, *mut c_void) -> c_int,
    disconnect: unsafe extern "C" fn() -> c_int,
    get_devices: unsafe extern "C" fn(*const CorsairDeviceFilter, c_int, *mut CorsairDeviceInfo, *mut c_int) -> c_int,
    get_led_positions: unsafe extern "C" fn(*const c_char, c_int, *mut CorsairLedPosition, *mut c_int) -> c_int,
    subscribe: unsafe extern "C" fn(EventCb, *mut c_void) -> c_int,
    read_property: unsafe extern "C" fn(*const c_char, c_int, c_uint, *mut CorsairProperty) -> c_int,
    free_property: unsafe extern "C" fn(*mut CorsairProperty) -> c_int,
    set_led_colors: unsafe extern "C" fn(*const c_char, c_int, *const CorsairLedColor) -> c_int,
    get_led_colors: unsafe extern "C" fn(*const c_char, c_int, *mut CorsairLedColor) -> c_int,
    set_layer_priority: unsafe extern "C" fn(c_uint) -> c_int,
}

#[derive(Debug, Clone)]
pub enum SdkEvent {
    Session { state: i32, icue_version: Option<String> },
    DevicesChanged,
}

#[derive(Debug, Clone)]
pub struct Keyboard {
    pub id: String,
    pub model: String,
    pub led_count: i32,
}

pub type EventSink = Box<dyn Fn(SdkEvent) + Send + Sync>;

pub struct Sdk {
    f: Fns,
    pub path: PathBuf,
    _lib: Library,
}

fn device_id(id: &str) -> DeviceId {
    let mut buf = [0 as c_char; STRING_SIZE_M];
    for (dst, src) in buf.iter_mut().zip(id.bytes().take(STRING_SIZE_M - 1)) {
        *dst = src as c_char;
    }
    buf
}

fn c_string(buf: &[c_char]) -> String {
    let bytes: Vec<u8> = buf.iter().take_while(|&&c| c != 0).map(|&c| c as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

extern "C" fn on_session(ctx: *mut c_void, data: *const CorsairSessionStateChanged) {
    if ctx.is_null() || data.is_null() {
        return;
    }
    let (sink, data) = unsafe { (&*(ctx as *const EventSink), &*data) };
    let h = data.details.host;
    let icue_version = (h.major > 0).then(|| format!("{}.{}.{}", h.major, h.minor, h.patch));
    sink(SdkEvent::Session { state: data.state, icue_version });
}

extern "C" fn on_event(ctx: *mut c_void, event: *const CorsairEvent) {
    if ctx.is_null() || event.is_null() {
        return;
    }
    let (sink, event) = unsafe { (&*(ctx as *const EventSink), &*event) };
    if event.id == CEI_DEVICE_CONNECTION_STATUS_CHANGED {
        sink(SdkEvent::DevicesChanged);
    }
}

impl Sdk {
    /// Loads the first existing SDK library from `candidates`.
    pub fn load(candidates: &[PathBuf]) -> Result<Sdk, String> {
        let mut errors = Vec::new();
        for path in candidates {
            if !path.exists() {
                continue;
            }
            match unsafe { Self::load_from(path) } {
                Ok(sdk) => return Ok(sdk),
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            }
        }
        if errors.is_empty() {
            Err("iCUE SDK library not found".into())
        } else {
            Err(errors.join("; "))
        }
    }

    /// Like [`Sdk::load`], but downloads the SDK from Corsair if it is still missing.
    pub fn load_or_download(candidates: &[PathBuf]) -> Result<Sdk, String> {
        Sdk::load(candidates).or_else(|_| download().and_then(|path| Sdk::load(&[path])))
    }

    unsafe fn load_from(path: &Path) -> Result<Sdk, libloading::Error> {
        let lib = Library::new(path)?;
        let f = Fns {
            connect: *lib.get(b"CorsairConnect\0")?,
            disconnect: *lib.get(b"CorsairDisconnect\0")?,
            get_devices: *lib.get(b"CorsairGetDevices\0")?,
            get_led_positions: *lib.get(b"CorsairGetLedPositions\0")?,
            subscribe: *lib.get(b"CorsairSubscribeForEvents\0")?,
            read_property: *lib.get(b"CorsairReadDeviceProperty\0")?,
            free_property: *lib.get(b"CorsairFreeProperty\0")?,
            set_led_colors: *lib.get(b"CorsairSetLedColors\0")?,
            get_led_colors: *lib.get(b"CorsairGetLedColors\0")?,
            set_layer_priority: *lib.get(b"CorsairSetLayerPriority\0")?,
        };
        Ok(Sdk { f, path: path.to_path_buf(), _lib: lib })
    }

    /// Starts the connection to iCUE. The SDK reconnects on its own when
    /// iCUE starts later or restarts. The sink lives until the process exits.
    pub fn connect(&self, sink: EventSink) -> Result<(), i32> {
        let ctx = Box::into_raw(Box::new(sink)) as *mut c_void;
        check(unsafe { (self.f.connect)(on_session, ctx) })
    }

    pub fn subscribe(&self, sink: EventSink) -> Result<(), i32> {
        let ctx = Box::into_raw(Box::new(sink)) as *mut c_void;
        check(unsafe { (self.f.subscribe)(on_event, ctx) })
    }

    pub fn disconnect(&self) {
        unsafe { (self.f.disconnect)() };
    }

    /// Layer priority relative to iCUE (127) and other SDK programs.
    pub fn set_layer_priority(&self, priority: u32) -> Result<(), i32> {
        check(unsafe { (self.f.set_layer_priority)(priority.min(255)) })
    }

    pub fn keyboards(&self) -> Result<Vec<Keyboard>, i32> {
        let filter = CorsairDeviceFilter { device_type_mask: CDT_KEYBOARD };
        let mut devices: Vec<CorsairDeviceInfo> = (0..DEVICE_COUNT_MAX)
            .map(|_| CorsairDeviceInfo {
                kind: 0,
                id: [0; STRING_SIZE_M],
                serial: [0; STRING_SIZE_M],
                model: [0; STRING_SIZE_M],
                led_count: 0,
                channel_count: 0,
            })
            .collect();
        let mut count: c_int = 0;
        check(unsafe {
            (self.f.get_devices)(&filter, DEVICE_COUNT_MAX as c_int, devices.as_mut_ptr(), &mut count)
        })?;
        Ok(devices
            .iter()
            .take(count.max(0) as usize)
            .map(|d| Keyboard { id: c_string(&d.id), model: c_string(&d.model), led_count: d.led_count })
            .collect())
    }

    /// LED positions `(luid, x_mm, y_mm)`.
    pub fn led_positions(&self, device: &str) -> Result<Vec<(u32, f64, f64)>, i32> {
        let id = device_id(device);
        let mut buf = vec![CorsairLedPosition::default(); LEDCOUNT_MAX];
        let mut count: c_int = 0;
        check(unsafe {
            (self.f.get_led_positions)(id.as_ptr(), LEDCOUNT_MAX as c_int, buf.as_mut_ptr(), &mut count)
        })?;
        Ok(buf.iter().take(count.max(0) as usize).map(|p| (p.id, p.cx, p.cy)).collect())
    }

    pub fn battery(&self, device: &str) -> Option<i32> {
        self.read_int(device, CDPI_BATTERY_LEVEL)
    }

    /// Logical key layout as a short code (`"de"`, `"us"`, `"uk"`, …), so the
    /// preview can label keys the way they are printed on the keycaps.
    pub fn logical_layout(&self, device: &str) -> Option<&'static str> {
        Some(match self.read_int(device, CDPI_LOGICAL_LAYOUT)? {
            1 | 2 | 18 => "us",
            4 => "uk",
            5 => "be",
            6 => "br",
            7 => "ch",
            9 => "de",
            10 => "es",
            11 => "fr",
            12 => "it",
            13 => "nordic",
            14 => "ru",
            15 => "jp",
            16 => "kr",
            _ => "us",
        })
    }

    fn read_int(&self, device: &str, property: c_int) -> Option<i32> {
        let id = device_id(device);
        let mut prop = CorsairProperty { kind: 0, value: [0; 2] };
        let err = unsafe { (self.f.read_property)(id.as_ptr(), property, 0, &mut prop) };
        if err != CE_SUCCESS {
            return None;
        }
        let value = prop.value[0] as u32 as i32;
        unsafe { (self.f.free_property)(&mut prop) };
        Some(value)
    }

    /// Sets colors; `alpha = 0` makes our layer transparent (iCUE shows through).
    pub fn set_colors(&self, device: &str, leds: &[(u32, [u8; 3])], alpha: u8) -> Result<(), i32> {
        let id = device_id(device);
        let colors: Vec<CorsairLedColor> = leds
            .iter()
            .map(|&(luid, [r, g, b])| CorsairLedColor { id: luid, r, g, b, a: alpha })
            .collect();
        check(unsafe { (self.f.set_led_colors)(id.as_ptr(), colors.len() as c_int, colors.as_ptr()) })
    }

    /// Actual colors as reported by the hardware (mix of iCUE and SDK layers).
    pub fn get_colors(&self, device: &str, luids: &[u32]) -> Result<Vec<[u8; 3]>, i32> {
        let id = device_id(device);
        let mut colors: Vec<CorsairLedColor> =
            luids.iter().map(|&l| CorsairLedColor { id: l, ..Default::default() }).collect();
        check(unsafe { (self.f.get_led_colors)(id.as_ptr(), colors.len() as c_int, colors.as_mut_ptr()) })?;
        Ok(colors.iter().map(|c| [c.r, c.g, c.b]).collect())
    }
}

fn check(err: c_int) -> Result<(), i32> {
    if err == CE_SUCCESS {
        Ok(())
    } else {
        Err(err)
    }
}

pub fn error_text(err: i32) -> &'static str {
    match err {
        1 => "not connected to iCUE",
        2 => "another program has exclusive control",
        3 => "iCUE version does not match the SDK",
        4 => "invalid arguments",
        5 => "action not allowed in the current state",
        6 => "device not found",
        7 => "not allowed in iCUE",
        _ => "unknown error",
    }
}

// The iCUE SDK belongs to Corsair, and its license doesn't allow
// redistribution. So the app doesn't ship it; instead it downloads it on first
// start on each machine from Corsair's official GitHub release and verifies
// the checksum.
const SDK_VERSION: &str = "4.0.84";
#[cfg(windows)]
const SDK_DOWNLOAD: (&str, &str) = (
    "https://github.com/CorsairOfficial/cue-sdk/releases/download/v4.0.84/iCUESDK_4.0.84.zip",
    "eb4414cf505145f3e507dc839ad587bf7ca684d64be3d9a834f416a136736d5d",
);
#[cfg(target_os = "macos")]
const SDK_DOWNLOAD: (&str, &str) = (
    "https://github.com/CorsairOfficial/cue-sdk/releases/download/v4.0.84/iCUESDK_4.0.84.dmg",
    "2b818c879036fdd5e654a89cc2296d315fca07b55772609219aba4540ee1812f",
);
/// Path of the Windows DLL inside Corsair's ZIP.
#[cfg(any(windows, test))]
const ZIP_DLL_PATH: &str = "iCUESDK/redist/x64/iCUESDK.x64_2019.dll";

#[cfg(any(windows, target_os = "macos"))]
const DATA_FOLDER: &str = "K65 Plus Wireless Color Settings";

/// Folder holding the downloaded SDK (per user, no admin rights needed).
fn sdk_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    let base = PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support").join(DATA_FOLDER);
    #[cfg(windows)]
    let base = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join(DATA_FOLDER);
    #[cfg(not(any(windows, target_os = "macos")))]
    return None;
    #[cfg(any(windows, target_os = "macos"))]
    Some(base.join(format!("icue-sdk-{SDK_VERSION}")))
}

/// The library in the SDK folder. On macOS the framework: the loose
/// `libiCUESDK.dylib` from Corsair's DMG has a broken arm64 signature and
/// gets killed by the system.
fn sdk_binary(dir: &Path) -> PathBuf {
    if cfg!(windows) {
        dir.join("iCUESDK.x64_2019.dll")
    } else {
        dir.join("iCUESDK.framework/Versions/A/iCUESDK")
    }
}

/// Where to look for the SDK library (without download).
pub fn default_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = std::env::var_os("K65_ICUE_SDK") {
        out.push(PathBuf::from(p));
    }
    if let Some(dir) = sdk_dir() {
        out.push(sdk_binary(&dir));
    }
    out
}

/// Downloads the SDK from Corsair if it is still missing and returns the path to the library.
#[cfg(any(windows, target_os = "macos"))]
pub fn download() -> Result<PathBuf, String> {
    use std::io::Read;
    let dir = sdk_dir().ok_or("no folder for app data found")?;
    let target = sdk_binary(&dir);
    if target.exists() {
        return Ok(target);
    }
    let (url, sha256) = SDK_DOWNLOAD;
    log::info!("Downloading iCUE SDK {SDK_VERSION} from {url}");
    // Native TLS (Security.framework or SChannel), no bundled crypto library.
    let tls = ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).build();
    let agent = ureq::Agent::new_with_config(ureq::Agent::config_builder().tls_config(tls).build());
    let mut bytes = Vec::new();
    agent
        .get(url)
        .call()
        .map_err(|e| format!("download failed: {e}"))?
        .into_body()
        .into_reader()
        .take(64 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("download interrupted: {e}"))?;
    if hex_sha256(&bytes) != sha256 {
        return Err("iCUE SDK checksum mismatch, download discarded".into());
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    #[cfg(windows)]
    {
        let dll = dll_from_zip(&bytes)?;
        let tmp = target.with_extension("part");
        std::fs::write(&tmp, dll).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    framework_from_dmg(&bytes, &dir)?;

    Ok(target)
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn download() -> Result<PathBuf, String> {
    Err("the iCUE SDK is only available for Windows and macOS".into())
}

#[cfg(any(windows, target_os = "macos"))]
fn hex_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(any(windows, test))]
fn dll_from_zip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut file = archive.by_name(ZIP_DLL_PATH).map_err(|e| format!("{ZIP_DLL_PATH}: {e}"))?;
    let mut dll = Vec::new();
    file.read_to_end(&mut dll).map_err(|e| e.to_string())?;
    Ok(dll)
}

/// Mounts the DMG read-only and copies the framework including its signature.
#[cfg(target_os = "macos")]
fn framework_from_dmg(bytes: &[u8], dir: &Path) -> Result<(), String> {
    use std::process::Command;
    let run = |cmd: &mut Command| -> Result<(), String> {
        let out = cmd.output().map_err(|e| e.to_string())?;
        out.status.success().then_some(()).ok_or_else(|| String::from_utf8_lossy(&out.stderr).trim().to_string())
    };
    let dmg = dir.join("sdk.dmg");
    let mount = dir.join("mnt");
    let staged = dir.join("iCUESDK.framework.part");
    std::fs::write(&dmg, bytes).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&staged);
    std::fs::create_dir_all(&mount).map_err(|e| e.to_string())?;
    let attached = run(Command::new("hdiutil").args(["attach", "-nobrowse", "-readonly", "-mountpoint"]).arg(&mount).arg(&dmg));
    let copied = attached.and_then(|_| run(Command::new("ditto").arg(mount.join("iCUESDK.framework")).arg(&staged)));
    let _ = run(Command::new("hdiutil").args(["detach", "-quiet"]).arg(&mount));
    let _ = std::fs::remove_file(&dmg);
    let _ = std::fs::remove_dir(&mount);
    copied?;
    std::fs::rename(&staged, dir.join("iCUESDK.framework")).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks extraction with Corsair's real ZIP if available locally
    /// (`K65_TEST_SDK_ZIP=…/iCUESDK_4.0.84.zip`); skipped otherwise.
    #[test]
    fn extracts_windows_dll_from_corsair_zip() {
        let Some(path) = std::env::var_os("K65_TEST_SDK_ZIP") else { return };
        let bytes = std::fs::read(path).unwrap();
        let dll = dll_from_zip(&bytes).unwrap();
        assert_eq!(&dll[..2], b"MZ", "Windows DLL");
        assert_eq!(dll.len(), 509_992);
    }

    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn sha256_hex() {
        assert_eq!(hex_sha256(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
