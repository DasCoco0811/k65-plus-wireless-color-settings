import type { Palette } from "./api";

// Keycap labels follow the keyboard's logical layout as reported by iCUE, so the
// preview shows what is printed on the keys. The SDK's LED names are US positions.
const LABELS: Record<string, string> = {
  Escape: "Esc",
  GraveAccentAndTilde: "`",
  MinusAndUnderscore: "-",
  EqualsAndPlus: "=",
  Backspace: "⌫",
  Tab: "⇥",
  BracketLeft: "[",
  BracketRight: "]",
  Enter: "↵",
  CapsLock: "⇪",
  SemicolonAndColon: ";",
  ApostropheAndDoubleQuote: "'",
  NonUsTilde: "#",
  Backslash: "\\",
  LeftShift: "⇧",
  RightShift: "⇧",
  NonUsBackslash: "\\",
  CommaAndLessThan: ",",
  PeriodAndBiggerThan: ".",
  SlashAndQuestionMark: "/",
  LeftCtrl: "Ctrl",
  RightCtrl: "Ctrl",
  Space: "",
  Fn: "Fn",
  Application: "☰",
  UpArrow: "↑",
  DownArrow: "↓",
  LeftArrow: "←",
  RightArrow: "→",
  Home: "Home",
  End: "End",
  PageUp: "PgUp",
  PageDown: "PgDn",
  Delete: "Del",
  Insert: "Ins",
  PrintScreen: "PrtSc",
  ScrollLock: "ScrLk",
  PauseBreak: "Pause",
  NumLock: "Num",
  KeypadSlash: "/",
  KeypadAsterisk: "*",
  KeypadMinus: "−",
  KeypadPlus: "+",
  KeypadEnter: "↵",
  KeypadPeriodAndDelete: ".",
  KeypadComma: ",",
};

// German ISO keycaps.
const LABELS_DE: Record<string, string> = {
  GraveAccentAndTilde: "^",
  MinusAndUnderscore: "ß",
  EqualsAndPlus: "´",
  Y: "Z",
  Z: "Y",
  BracketLeft: "Ü",
  BracketRight: "+",
  SemicolonAndColon: "Ö",
  ApostropheAndDoubleQuote: "Ä",
  NonUsBackslash: "<",
  SlashAndQuestionMark: "-",
  LeftCtrl: "Strg",
  RightCtrl: "Strg",
  RightAlt: "AltGr",
  Home: "Pos1",
  End: "Ende",
  PageUp: "Bild↑",
  PageDown: "Bild↓",
  Delete: "Entf",
  Insert: "Einfg",
  PrintScreen: "Druck",
  ScrollLock: "Rollen",
  KeypadPeriodAndDelete: ",",
};

let layout = "us";

/** Sets the logical layout of the connected keyboard (`"de"`, `"us"`, …). */
export function setKeyboardLayout(code: string | null | undefined) {
  layout = code ?? "us";
}

// Media and special keys without their own LED, as the app reports them.
const TOKENS: Record<string, string> = {
  VolumeUp: "Volume +",
  VolumeDown: "Volume −",
  Mute: "Mute",
  PlayPause: "Play/Pause",
  Next: "Next track",
  Previous: "Previous track",
  Stop: "Stop",
  Fast: "Fast forward",
  Rewind: "Rewind",
  Eject: "Eject",
  BrightnessUp: "Brightness +",
  BrightnessDown: "Brightness −",
  IlluminationUp: "Keyboard light +",
  IlluminationDown: "Keyboard light −",
  IlluminationToggle: "Keyboard light on/off",
  MissionControl: "Mission Control",
  Launchpad: "Launchpad",
  Spotlight: "Spotlight",
  Dictation: "Dictation",
  DoNotDisturb: "Do Not Disturb",
};

export function tokenLabel(token: string): string {
  if (TOKENS[token]) return TOKENS[token];
  const code = token.match(/^(?:mac|win|nx):(.+)$/);
  return code ? `Special key ${code[1]}` : token;
}

// Order and spelling as in the Rust core (combo.rs).
const MODS: [number, string][] = [
  [2, "Ctrl"],
  [4, "Alt"],
  [1, "Shift"],
  [8, "Gui"],
];

export function comboName(mods: number, key: string): string {
  return [...MODS.filter(([bit]) => mods & bit).map(([, n]) => n), key].join("+");
}

export function parseCombo(combo: string): { mods: number; key: string } | null {
  const parts = combo.split("+");
  if (parts.length < 2) return null;
  let mods = 0;
  for (const p of parts.slice(0, -1)) {
    const m = MODS.find(([, n]) => n === p);
    if (!m) return null;
    mods |= m[0];
  }
  return { mods, key: parts[parts.length - 1] };
}

export function modLabel(bit: number, platform: string): string {
  const mac = platform === "macos";
  const ctrl = layout === "de" ? "Strg" : "Ctrl";
  return ({ 1: mac ? "⇧" : "Shift", 2: mac ? "⌃" : ctrl, 4: mac ? "⌥" : "Alt", 8: mac ? "⌘" : "Win" } as Record<number, string>)[bit];
}

export function comboLabel(combo: string, platform: string): string {
  const parsed = parseCombo(combo);
  if (!parsed) return combo;
  const mods = MODS.filter(([bit]) => parsed.mods & bit).map(([bit]) => modLabel(bit, platform));
  return [...mods, keyLabel(parsed.key, platform) || "Space"].join(" + ");
}

/** Text for "Last key": an LED key, a combo or a media/special key. */
export function displayKey(name: string, platform: string): string {
  if (TOKENS[name] || /^(?:mac|win|nx):/.test(name)) return tokenLabel(name);
  if (parseCombo(name)) return comboLabel(name, platform);
  return keyLabel(name, platform) || "Space";
}

export function keyLabel(name: string, platform: string): string {
  const mac = platform === "macos";
  if (name === "LeftGui" || name === "RightGui") return mac ? "⌘" : "Win";
  if (name === "LeftAlt") return mac ? "⌥" : "Alt";
  if (name.startsWith("Keypad") && name.length === 7) return name.slice(6);
  if (layout === "de" && LABELS_DE[name] !== undefined) return LABELS_DE[name];
  if (name === "RightAlt") return mac ? "⌥" : "Alt";
  return LABELS[name] ?? name;
}

// Tones picked so they stay clearly apart as LED light (see colors.ts):
// every preset keeps an OKLab distance of at least 0.2 between all three colors.
const PINK = "#FF33C0";
const TEAL = "#00E0D0";

export const PRESETS: { name: string; palette: Palette }[] = [
  { name: "Shibuya", palette: { base: "#9722B5", primary: "#FF2172", secondary: TEAL } },
  { name: "Galaxy", palette: { base: "#14003D", primary: "#942192", secondary: "#00E5FF" } },
  { name: "Synthwave", palette: { base: "#2B0A57", primary: "#FF2E97", secondary: "#00E5FF" } },
  { name: "Night", palette: { base: "#2E0F57", primary: PINK, secondary: TEAL } },
  { name: "Aurora", palette: { base: "#00FF9C", primary: "#7A2BFF", secondary: "#FF2E97" } },
  { name: "Sunset", palette: { base: "#FFA200", primary: "#FF1F6E", secondary: "#6A2BFF" } },
  { name: "Cyberpunk", palette: { base: "#FCEE0A", primary: "#00F0FF", secondary: "#FF003C" } },
  { name: "Ocean", palette: { base: "#0030FF", primary: TEAL, secondary: "#FFFFFF" } },
  { name: "Ice", palette: { base: "#0A1A66", primary: "#00E5FF", secondary: "#FFFFFF" } },
  { name: "Ember", palette: { base: "#FF2200", primary: "#FFB000", secondary: "#FFFFFF" } },
  { name: "Toxic", palette: { base: "#00FF55", primary: "#FF00D4", secondary: "#FFC400" } },
  { name: "Strawberry", palette: { base: "#FF0A3C", primary: "#FFFFFF", secondary: "#22FF55" } },
];

export const rgb = ([r, g, b]: [number, number, number]) => `rgb(${r},${g},${b})`;

/** Perceived brightness 0–1, for readable labels on colored keys. */
export const luminance = ([r, g, b]: [number, number, number]) => (0.299 * r + 0.587 * g + 0.114 * b) / 255;
