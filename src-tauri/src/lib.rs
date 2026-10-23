//! Tauri shell: menu bar/tray icon, settings window and the commands
//! through which the UI talks to the light service.

use std::path::PathBuf;
use std::sync::Mutex;
use k65_color_core::config::{self, Config, ExportFile, Library};
use k65_color_core::input;
use k65_color_core::sdk;
use k65_color_core::service::{Command, Service, Snapshot};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WindowEvent, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_dialog::DialogExt;

const HIDDEN_ARG: &str = "--hidden";
const APP_NAME: &str = "K65 Plus Wireless Color Settings";

struct AppState {
    service: Service,
    config: Mutex<Config>,
    config_path: PathBuf,
    library: Mutex<Library>,
    library_path: PathBuf,
    pause_item: CheckMenuItem<Wry>,
}

fn apply_config(state: &AppState, cfg: Config) -> Result<(), String> {
    config::save(&state.config_path, &cfg).map_err(|e| format!("Saving failed: {e}"))?;
    let _ = state.pause_item.set_checked(cfg.paused);
    *state.config.lock().unwrap() = cfg.clone();
    state.service.send(Command::SetConfig(cfg));
    Ok(())
}

#[tauri::command]
fn get_snapshot(state: State<AppState>) -> Snapshot {
    state.service.snapshot()
}

#[tauri::command]
fn get_config(state: State<AppState>) -> Config {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn set_config(state: State<AppState>, config: Config) -> Result<(), String> {
    apply_config(&state, config)
}

#[tauri::command]
fn default_config() -> Config {
    Config::default()
}

#[tauri::command]
fn get_library(state: State<AppState>) -> Library {
    state.library.lock().unwrap().clone()
}

#[tauri::command]
fn set_library(state: State<AppState>, library: Library) -> Result<(), String> {
    config::save(&state.library_path, &library).map_err(|e| format!("Saving failed: {e}"))?;
    *state.library.lock().unwrap() = library;
    Ok(())
}

/// Saves all custom profiles and color sets to a file. Returns the file name.
#[tauri::command]
async fn export_profiles(app: AppHandle) -> Result<Option<String>, String> {
    let file = app.state::<AppState>().library.lock().unwrap().export();
    let Some(path) = app
        .dialog()
        .file()
        .set_title("Export profiles")
        .set_file_name("k65-plus-wireless-profiles.json")
        .add_filter("K65 color profiles", &["json"])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    config::save(&path, &file).map_err(|e| format!("Export failed: {e}"))?;
    Ok(path.file_name().map(|n| n.to_string_lossy().into_owned()))
}

#[derive(serde::Serialize)]
struct ImportResult {
    profiles: usize,
    palettes: usize,
    library: Library,
}

/// Adds profiles and color sets from an exported file.
#[tauri::command]
async fn import_profiles(app: AppHandle) -> Result<Option<ImportResult>, String> {
    let Some(path) = app
        .dialog()
        .file()
        .set_title("Import profiles")
        .add_filter("K65 color profiles", &["json"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Cannot read the file: {e}"))?;
    let file: ExportFile =
        serde_json::from_str(&text).map_err(|_| "This is not a K65 Plus Wireless Color Settings profile file".to_string())?;
    let state = app.state::<AppState>();
    let mut library = state.library.lock().unwrap();
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let (profiles, palettes) = library.import(file, seed)?;
    config::save(&state.library_path, &*library).map_err(|e| format!("Saving failed: {e}"))?;
    Ok(Some(ImportResult { profiles, palettes, library: library.clone() }))
}

#[tauri::command]
fn press_key(state: State<AppState>, name: String, mods: Option<u8>) {
    state.service.send(Command::Press { name, mods: mods.unwrap_or(0) });
}

#[tauri::command]
fn reset_all(state: State<AppState>) {
    state.service.send(Command::ResetAll);
}

#[tauri::command]
fn show_palette(state: State<AppState>) {
    state.service.send(Command::ShowPalette);
}

#[tauri::command]
fn request_input_access() -> bool {
    input::request_permission()
}

#[tauri::command]
fn open_input_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn platform() -> &'static str {
    std::env::consts::OS
}

/// Switches start-at-login on once, so the lighting is back after a restart
/// without opening the app by hand.
fn set_up_autostart(app: &AppHandle, cfg: &mut Config, config_path: &std::path::Path) {
    if cfg.autostart_set_up {
        return;
    }
    // Started straight from the disk image or a translocated copy: the login
    // item would point to a path that disappears. Wait for the installed app.
    let exe = std::env::current_exe().unwrap_or_default();
    let temporary = exe.starts_with("/Volumes/") || exe.to_string_lossy().contains("/AppTranslocation/");
    if temporary {
        log::info!("Not enabling start at login from a temporary location: {}", exe.display());
        return;
    }
    match app.autolaunch().enable() {
        Ok(()) => {
            cfg.autostart_set_up = true;
            if let Err(e) = config::save(config_path, cfg) {
                log::warn!("Could not save config: {e}");
            }
        }
        Err(e) => log::warn!("Could not enable start at login: {e}"),
    }
}

fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn toggle_pause(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut cfg = state.config.lock().unwrap().clone();
    cfg.paused = !cfg.paused;
    match apply_config(&state, cfg.clone()) {
        // Only report changes from the menu; the UI already knows about its own saves.
        Ok(()) => drop(app.emit("config-changed", cfg)),
        Err(e) => log::error!("{e}"),
    }
}

fn build_tray(app: &tauri::App, paused: bool) -> tauri::Result<CheckMenuItem<Wry>> {
    let open = MenuItem::with_id(app, "open", format!("Open {APP_NAME}"), true, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, "pause", "Pause (show iCUE lighting)", true, paused, None::<&str>)?;
    let reset = MenuItem::with_id(app, "reset", "Reset all zones", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &pause, &reset, &sep, &quit])?;

    TrayIconBuilder::with_id("tray")
        .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .icon_as_template(true)
        .tooltip(APP_NAME)
        .menu(&menu)
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_window(app),
            "pause" => toggle_pause(app),
            "reset" => app.state::<AppState>().service.send(Command::ResetAll),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                if !cfg!(target_os = "macos") {
                    show_window(tray.app_handle());
                }
            }
        })
        .build(app)?;
    Ok(pause)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_window(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![HIDDEN_ARG])))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_config,
            set_config,
            default_config,
            get_library,
            set_library,
            export_profiles,
            import_profiles,
            press_key,
            reset_all,
            show_palette,
            request_input_access,
            open_input_settings,
            platform,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let dir = app.path().app_config_dir()?;
            let config_path = dir.join("config.json");
            let library_path = dir.join("library.json");
            let mut cfg: Config = config::load(&config_path);
            let library: Library = config::load(&library_path);
            log::info!("Config: {}", config_path.display());
            set_up_autostart(app.handle(), &mut cfg, &config_path);
            let service = Service::start(cfg.clone(), sdk::default_candidates());
            let pause_item = build_tray(app, cfg.paused)?;
            app.manage(AppState {
                service,
                config: Mutex::new(cfg),
                config_path,
                library: Mutex::new(library),
                library_path,
                pause_item,
            });

            if !std::env::args().any(|a| a == HIDDEN_ARG) {
                show_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing only hides the window; the lighting keeps running.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("K65 Plus Wireless Color Settings could not start");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            if let Some(state) = app.try_state::<AppState>() {
                state.service.shutdown();
            }
        }
    });
}
