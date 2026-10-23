import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Palette = { base: string; primary: string; secondary: string };

export type Zone = {
  id: string;
  name: string;
  enabled: boolean;
  keys: string[];
  own_colors: boolean;
  palette: Palette;
};

export type Action =
  | { type: "ring" | "reset_all" | "wave" | "sweep" | "flash" | "sparkle" | "rise" }
  | { type: "reset_zones" | "cycle_zones" | "wipe"; zones: string[] };

/** Modifier bits as in the Rust core (combo.rs). */
export const MOD = { shift: 1, ctrl: 2, alt: 4, gui: 8 } as const;

export type Config = {
  palette: Palette;
  brightness: number;
  timeout_secs: number;
  reach: number;
  press_fade_ms: number;
  reset_fade_ms: number;
  swap_alt_gui: boolean;
  led_correction: boolean;
  paused: boolean;
  zones: Zone[];
  key_actions: Record<string, Action>;
  /** Key combos such as "Gui+Backspace" → action. */
  combo_actions: Record<string, Action>;
  /** Learned media/special keys: token → LED name ("" = ignore). */
  learned_keys: Record<string, string>;
  /** Background effect while no key is colored. */
  ambient: Ambient;
  /** Whether start-at-login was switched on once (managed by the app). */
  autostart_set_up: boolean;
};

export type AmbientMode = "off" | "aurora" | "wave" | "fade" | "breathe" | "glimmer";
export type Ambient = { mode: AmbientMode; speed: number; intensity: number };

export type NamedPalette = { name: string; palette: Palette };
export type Profile = { id: string; name: string; config: Config };
export type Library = { palettes: NamedPalette[]; profiles: Profile[]; active_profile: string | null };

export type Led = { name: string; x: number; y: number; w: number; h: number; zone: number | null };

export type InputStatus =
  | { state: "starting" | "running" | "no_permission" | "unsupported" }
  | { state: "failed"; detail: string };

export type Snapshot = {
  icue: "loading_sdk" | "missing_sdk" | "connecting" | "connected" | "unavailable" | "refused";
  icue_version: string | null;
  sdk_error: string | null;
  device: { model: string; led_count: number; battery: number | null; layout: string | null } | null;
  input: InputStatus;
  input_permission: boolean | null;
  leds: Led[];
  colors: [number, number, number][];
  last_key: string | null;
  unknown_key: string | null;
  paused: boolean;
};

export const api = {
  snapshot: () => invoke<Snapshot>("get_snapshot"),
  config: () => invoke<Config>("get_config"),
  defaults: () => invoke<Config>("default_config"),
  library: () => invoke<Library>("get_library"),
  saveLibrary: (library: Library) => invoke<void>("set_library", { library }),
  exportProfiles: () => invoke<string | null>("export_profiles"),
  importProfiles: () => invoke<{ profiles: number; palettes: number; library: Library } | null>("import_profiles"),
  saveConfig: (config: Config) => invoke<void>("set_config", { config }),
  press: (name: string, mods = 0) => invoke<void>("press_key", { name, mods }),
  resetAll: () => invoke<void>("reset_all"),
  showPalette: () => invoke<void>("show_palette"),
  requestInput: () => invoke<boolean>("request_input_access"),
  openInputSettings: () => invoke<void>("open_input_settings"),
  platform: () => invoke<string>("platform"),
  onConfigChanged: (fn: (c: Config) => void) => listen<Config>("config-changed", (e) => fn(e.payload)),
};
