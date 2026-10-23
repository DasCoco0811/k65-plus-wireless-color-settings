import { useEffect, useMemo, useRef, useState } from "react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { api, type Config, type Library, type Snapshot } from "./api";
import { Keyboard } from "./components/Keyboard";
import { ColorField, Slider, Toggle } from "./components/Controls";
import { Zones } from "./components/Zones";
import { SpecialKeys } from "./components/SpecialKeys";
import { Combos } from "./components/Combos";
import { AmbientPicker } from "./components/AmbientPicker";
import { displayKey, keyLabel, setKeyboardLayout, tokenLabel } from "./keys";
import { Palettes } from "./components/Palettes";
import { Profiles } from "./components/Profiles";
import { similarPairs } from "./colors";

export const APP_NAME = "K65 Plus Wireless Color Settings";
const POLL_MS = 60;
const SAVE_DELAY_MS = 180;

const newId = () => `p-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;
const asProfileConfig = (c: Config): Config => ({ ...c, paused: false });

function formatTimeout(v: number) {
  if (v === 0) return "never";
  if (v < 60) return `${v} s`;
  const s = v % 60;
  return `${Math.floor(v / 60)} min${s ? ` ${s} s` : ""}`;
}

function Chip({ ok, warn, children }: { ok: boolean; warn?: boolean; children: React.ReactNode }) {
  return <span className={`chip ${ok ? "chip-ok" : warn ? "chip-warn" : "chip-bad"}`}>{children}</span>;
}

export default function App() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [cfg, setCfg] = useState<Config | null>(null);
  const [defaults, setDefaults] = useState<Config | null>(null);
  const [library, setLibrary] = useState<Library>({ palettes: [], profiles: [], active_profile: null });
  const [platform, setPlatform] = useState("macos");
  const [editZone, setEditZone] = useState<number | null>(null);
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const saveTimer = useRef<number | undefined>(undefined);
  // Library with an auto-saved profile that is still waiting to be written.
  const pendingLibrary = useRef<Library | null>(null);

  useEffect(() => {
    api.config().then(setCfg);
    api.defaults().then(setDefaults);
    api.library().then((lib) => {
      // Older profiles without an id get one, so renaming and the active state stay stable.
      const profiles = lib.profiles.map((p) => (p.id ? p : { ...p, id: newId() }));
      const fixed = { ...lib, profiles };
      setLibrary(fixed);
      if (profiles.some((p, i) => p !== lib.profiles[i])) api.saveLibrary(fixed);
    });
    api.platform().then(setPlatform);
    isEnabled().then(setAutostart).catch(() => setAutostart(null));
    const unlisten = api.onConfigChanged(setCfg);
    return () => void unlisten.then((f) => f());
  }, []);

  useEffect(() => {
    let alive = true;
    let timer: number | undefined;
    const tick = async () => {
      if (!alive) return;
      if (document.visibilityState === "visible") {
        try {
          setSnap(await api.snapshot());
        } catch (e) {
          setError(String(e));
        }
      }
      timer = window.setTimeout(tick, POLL_MS);
    };
    tick();
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, []);

  /** Applies new settings; if one of your profiles is active, it is saved along. */
  const update = (next: Config, lib: Library = library) => {
    setCfg(next);
    const active = lib.profiles.some((p) => p.id === lib.active_profile) ? lib.active_profile : null;
    if (active) {
      const withProfile = {
        ...lib,
        profiles: lib.profiles.map((p) => (p.id === active ? { ...p, config: asProfileConfig(next) } : p)),
      };
      setLibrary(withProfile);
      pendingLibrary.current = withProfile;
    }
    window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      api.saveConfig(next).then(() => setError(null), (e) => setError(String(e)));
      if (pendingLibrary.current) {
        api.saveLibrary(pendingLibrary.current).catch((e) => setError(String(e)));
        pendingLibrary.current = null;
      }
    }, SAVE_DELAY_MS);
  };

  /** Writes pending saves right away (e.g. before an export). */
  const flushSaves = async () => {
    window.clearTimeout(saveTimer.current);
    if (pendingLibrary.current) {
      await api.saveLibrary(pendingLibrary.current);
      pendingLibrary.current = null;
    }
  };

  const exportProfiles = async () => {
    try {
      await flushSaves();
      const file = await api.exportProfiles();
      if (file) setNotice(`Profiles and color sets saved to ${file}`);
    } catch (e) {
      setError(String(e));
    }
  };

  const importProfiles = async () => {
    try {
      await flushSaves();
      const result = await api.importProfiles();
      if (!result) return;
      setLibrary(result.library);
      setNotice(
        result.profiles + result.palettes === 0
          ? "Nothing new in that file, everything is already here."
          : `Imported ${result.profiles} profile${result.profiles === 1 ? "" : "s"} and ${result.palettes} color set${result.palettes === 1 ? "" : "s"}.`,
      );
    } catch (e) {
      setError(String(e));
    }
  };

  /** Saves structural library changes right away. */
  const saveLibrary = (next: Library) => {
    pendingLibrary.current = null;
    setLibrary(next);
    api.saveLibrary(next).then(() => setError(null), (e) => setError(String(e)));
  };

  const zoneOfKey = useMemo(() => {
    const map = new Map<string, number>();
    cfg?.zones.forEach((z, i) => z.keys.forEach((k) => map.has(k) || map.set(k, i)));
    return map;
  }, [cfg]);

  const present = useMemo(() => new Set(snap?.leds.map((l) => l.name) ?? []), [snap?.leds]);

  if (!cfg || !snap || !defaults) return <div className="loading">Starting {APP_NAME} …</div>;
  setKeyboardLayout(snap.device?.layout);

  // Unknown media/special key that the user can now assign to a key.
  const learning = snap.unknown_key && !(snap.unknown_key in cfg.learned_keys) ? snap.unknown_key : null;
  const learn = (token: string, led: string) => update({ ...cfg, learned_keys: { ...cfg.learned_keys, [token]: led } });

  const onKey = (name: string) => {
    if (editZone === null && learning) {
      learn(learning, name);
      api.press(name);
      return;
    }
    if (editZone === null) {
      api.press(name);
      return;
    }
    const inZone = cfg.zones[editZone].keys.includes(name);
    const zones = cfg.zones.map((z, i) => {
      if (i === editZone) return { ...z, keys: inZone ? z.keys.filter((k) => k !== name) : [...z.keys, name] };
      return inZone ? z : { ...z, keys: z.keys.filter((k) => k !== name) };
    });
    update({ ...cfg, zones });
  };

  const toggleAutostart = async (on: boolean) => {
    try {
      await (on ? enable() : disable());
      setAutostart(await isEnabled());
    } catch (e) {
      setError(String(e));
    }
  };

  const icueText = {
    connected: `iCUE ${snap.icue_version ?? ""}`,
    connecting: "Connecting to iCUE …",
    unavailable: "iCUE not running",
    refused: "iCUE refused",
    missing_sdk: "SDK missing",
    loading_sdk: "Loading iCUE SDK …",
  }[snap.icue];
  const inputOk = snap.input.state === "running";
  const editing = editZone !== null ? cfg.zones[editZone] : null;
  const similar = similarPairs(cfg.palette, cfg.led_correction);

  return (
    <div className="app">
      <header className="top">
        <div className="brand">
          <span className="logo" aria-hidden />
          <span>{APP_NAME}</span>
        </div>
        <div className="chips">
          <Chip ok={snap.icue === "connected"} warn={snap.icue === "connecting" || snap.icue === "loading_sdk"}>
            {icueText}
          </Chip>
          <Chip ok={!!snap.device}>
            {snap.device
              ? `${snap.device.model}${snap.device.battery != null ? ` · ${snap.device.battery} %` : ""}`
              : "No keyboard"}
          </Chip>
          <Chip ok={inputOk} warn={snap.input.state === "starting"}>
            Key detection
          </Chip>
        </div>
        <button className={`btn ${cfg.paused ? "btn-accent" : ""}`} onClick={() => update({ ...cfg, paused: !cfg.paused })}>
          {cfg.paused ? "Resume" : "Pause"}
        </button>
      </header>

      <div className="banners">
        {snap.icue === "loading_sdk" && (
          <div className="banner">First start: downloading the iCUE SDK once from Corsair's GitHub …</div>
        )}
        {snap.icue === "missing_sdk" && (
          <div className="banner banner-bad">
            The iCUE SDK could not be loaded: {snap.sdk_error}. It is downloaded from Corsair's GitHub on first start, which needs an
            internet connection once.
          </div>
        )}
        {snap.icue === "unavailable" && (
          <div className="banner">iCUE is not running. Start iCUE and the app connects on its own.</div>
        )}
        {snap.icue === "refused" && (
          <div className="banner">
            iCUE does not let other programs control the lighting. Enable SDK control in the iCUE settings.
          </div>
        )}
        {snap.icue === "connected" && !snap.device && (
          <div className="banner">
            No Corsair keyboard found. Connect it by cable or the wireless receiver; iCUE does not control lighting over Bluetooth.
          </div>
        )}
        {(snap.input.state === "no_permission" || snap.input_permission === false) && (
          <div className="banner banner-action">
            <span>Key presses are not detected yet. On macOS the app needs the “Input Monitoring” permission.</span>
            <span className="banner-buttons">
              <button className="btn btn-small btn-accent" onClick={() => api.requestInput()}>
                Request access
              </button>
              <button className="btn btn-small" onClick={() => api.openInputSettings()}>
                System Settings
              </button>
            </span>
          </div>
        )}
        {snap.input.state === "failed" && <div className="banner banner-bad">Key detection: {snap.input.detail}</div>}
        {learning && editZone === null && (
          <div className="banner banner-action banner-learn">
            <span>
              <b>{tokenLabel(learning)}</b> detected, but it is not assigned to a key yet. Click the key in the preview that sent it.
            </span>
            <span className="banner-buttons">
              <button className="btn btn-small" onClick={() => learn(learning, "")}>
                Ignore
              </button>
            </span>
          </div>
        )}
        {cfg.paused && <div className="banner">Paused: the keyboard shows your iCUE lighting.</div>}
        {error && <div className="banner banner-bad">{error}</div>}
      </div>

      <section className={`stage ${editing ? "stage-editing" : ""}`}>
        <Keyboard
          leds={snap.leds}
          colors={snap.colors}
          platform={platform}
          editZone={editZone}
          zoneOfKey={zoneOfKey}
          lastKey={snap.last_key}
          onKey={onKey}
        />
        <div className="stage-foot">
          {editing ? (
            <span>
              Select keys for <b>{editing.name}</b>: a click adds or removes a key. Numbers mark other zones.
            </span>
          ) : (
            <span>Clicking a key acts like a real key press.</span>
          )}
          <span className="last-key">
            Last key <b>{snap.last_key ? displayKey(snap.last_key, platform) : "–"}</b>
          </span>
          {editing ? (
            <button className="btn btn-small btn-accent" onClick={() => setEditZone(null)}>
              Done
            </button>
          ) : (
            <button className="btn btn-small" onClick={() => api.resetAll()}>
              Reset all
            </button>
          )}
        </div>
      </section>

      <main className="grid">
        <section className="panel panel-wide">
          <h2>Profiles</h2>
          <p className="panel-intro">
            A profile stores everything: colors, timing, zones, special keys and combos. Click to load, ✎ renames, ⧉ duplicates.
          </p>
          <Profiles
            current={cfg}
            defaults={defaults}
            library={library}
            onLoad={(config, id) => {
              const lib = { ...library, active_profile: id };
              saveLibrary(lib);
              update({ ...config, paused: cfg.paused }, lib);
            }}
            onCreate={(name) => {
              const id = newId();
              saveLibrary({ ...library, profiles: [...library.profiles, { id, name, config: asProfileConfig(cfg) }], active_profile: id });
            }}
            onRename={(id, name) => saveLibrary({ ...library, profiles: library.profiles.map((p) => (p.id === id ? { ...p, name } : p)) })}
            onDuplicate={(name, config) => {
              const copy = { id: newId(), name: `${name} (copy)`, config: asProfileConfig(config) };
              const at = library.profiles.findIndex((p) => p.name === name && p.config === config);
              const profiles = [...library.profiles];
              profiles.splice(at < 0 ? profiles.length : at + 1, 0, copy);
              const lib = { ...library, profiles, active_profile: copy.id };
              saveLibrary(lib);
              update({ ...copy.config, paused: cfg.paused }, lib);
            }}
            onDelete={(id) =>
              saveLibrary({
                ...library,
                profiles: library.profiles.filter((p) => p.id !== id),
                active_profile: library.active_profile === id ? null : library.active_profile,
              })
            }
          />
          <div className="profile-io">
            <button className="btn btn-small" onClick={exportProfiles} disabled={library.profiles.length + library.palettes.length === 0}>
              Export
            </button>
            <button className="btn btn-small" onClick={importProfiles}>
              Import
            </button>
            <span className="special-hint">
              {notice ?? "Takes all your profiles and color sets along in one file, e.g. to another computer."}
            </span>
          </div>
        </section>

        <section className="panel">
          <h2>Colors</h2>
          <Palettes
            current={cfg.palette}
            custom={library.palettes}
            onPick={(palette) => update({ ...cfg, palette })}
            onSaveCustom={(name) => saveLibrary({ ...library, palettes: [...library.palettes, { name, palette: cfg.palette }] })}
            onDeleteCustom={(i) => saveLibrary({ ...library, palettes: library.palettes.filter((_, j) => j !== i) })}
          />
          <div className="colors">
            <ColorField
              label="Base color"
              hint="All keys at rest"
              value={cfg.palette.base}
              onChange={(v) => update({ ...cfg, palette: { ...cfg.palette, base: v } })}
            />
            <ColorField
              label="Ring 1"
              hint="Around a pressed key"
              value={cfg.palette.primary}
              onChange={(v) => update({ ...cfg, palette: { ...cfg.palette, primary: v } })}
            />
            <ColorField
              label="Ring 2"
              hint="Alternates with ring 1"
              value={cfg.palette.secondary}
              onChange={(v) => update({ ...cfg, palette: { ...cfg.palette, secondary: v } })}
            />
          </div>
          {similar.length > 0 && (
            <p className="similar">
              {similar.join(", ")} {similar.length === 1 ? "is" : "are"} hard to tell apart on the keyboard. Pick a tone with more
              contrast or a preset.
            </p>
          )}
          <button className="btn btn-small" onClick={() => api.showPalette()} disabled={!snap.device || cfg.paused}>
            Compare on the keyboard
          </button>
          <p className="rule">
            Pressing a key gives the keys around it <b>ring 1</b>. A neighbor that is already colored moves on: ring 1 becomes ring 2
            and back. Pressing a colored key turns it back to the base color, and its free neighbors get the <b>other</b> ring color.{" "}
            {cfg.timeout_secs === 0
              ? "Zones stay colored until you reset them."
              : `A zone you leave alone for ${formatTimeout(cfg.timeout_secs)} returns to the base color.`}
          </p>
        </section>

        <section className="panel">
          <h2>Behavior</h2>
          <Slider
            label="Back to base color after"
            value={cfg.timeout_secs}
            min={0}
            max={300}
            step={5}
            format={formatTimeout}
            onChange={(v) => update({ ...cfg, timeout_secs: v })}
          />
          <Slider
            label="Ring size"
            value={cfg.reach}
            min={1}
            max={3}
            format={(v) => (v === 1 ? "direct neighbors" : `${v} rings`)}
            onChange={(v) => update({ ...cfg, reach: v })}
          />
          <Slider label="Brightness" value={cfg.brightness} min={10} max={100} format={(v) => `${v} %`} onChange={(v) => update({ ...cfg, brightness: v })} />
          <Slider
            label="Fade in"
            value={cfg.press_fade_ms}
            min={0}
            max={400}
            step={10}
            format={(v) => (v === 0 ? "instant" : `${v} ms`)}
            onChange={(v) => update({ ...cfg, press_fade_ms: v })}
          />
          <Slider
            label="Fade out on reset"
            value={cfg.reset_fade_ms}
            min={0}
            max={3000}
            step={50}
            format={(v) => (v === 0 ? "instant" : `${(v / 1000).toFixed(1)} s`)}
            onChange={(v) => update({ ...cfg, reset_fade_ms: v })}
          />
          <div className="toggles">
            <Toggle
              label="LED color correction"
              hint="Converts colors for the LEDs so the keyboard shows them like this screen"
              checked={cfg.led_correction}
              onChange={(v) => update({ ...cfg, led_correction: v })}
            />
            {autostart !== null && <Toggle label="Start at login" checked={autostart} onChange={toggleAutostart} />}
            <Toggle
              label={platform === "macos" ? "Swap ⌥ and ⌘" : "Swap Alt and Win"}
              hint="If the keyboard swaps these keys itself in Mac mode"
              checked={cfg.swap_alt_gui}
              onChange={(v) => update({ ...cfg, swap_alt_gui: v })}
            />
          </div>
          <button className="btn btn-small btn-ghost" onClick={() => api.defaults().then((d) => update({ ...d, paused: cfg.paused }))}>
            Reset to defaults
          </button>
        </section>

        <section className="panel panel-wide">
          <h2>Background</h2>
          <p className="panel-intro">
            While no key is colored, e.g. after Enter, Space or the automatic reset, an effect made from your three colors plays. As
            soon as a ring lights up, the background turns static again.
          </p>
          <AmbientPicker
            ambient={cfg.ambient}
            palette={cfg.palette}
            onChange={(ambient) => update({ ...cfg, ambient })}
            onShow={() => api.resetAll()}
          />
        </section>

        <section className="panel panel-wide">
          <h2>Zones</h2>
          <p className="panel-intro">Rings stay inside their zone. Each zone resets on its own.</p>
          <Zones
            zones={cfg.zones}
            present={present}
            editZone={editZone}
            globalPalette={cfg.palette}
            ledCorrection={cfg.led_correction}
            onChange={(zones) => update({ ...cfg, zones })}
            onEdit={setEditZone}
          />
        </section>

        <section className="panel panel-wide">
          <h2>Special keys</h2>
          <p className="panel-intro">
            These keys trigger an action instead of a ring. Effects run briefly across the whole keyboard and keep your colors underneath.
            Modifiers like {platform === "macos" ? "⌘, ⌥, ⌃" : "Win, Alt, Ctrl"} and Shift only fire when tapped alone, not while you hold
            them for a combo or a capital letter.
          </p>
          <SpecialKeys
            actions={cfg.key_actions}
            zones={cfg.zones}
            present={present}
            platform={platform}
            onChange={(key_actions) => update({ ...cfg, key_actions })}
            onTry={(key) => api.press(key)}
          />
          <h3 className="sub-head">Key combos</h3>
          <Combos
            combos={cfg.combo_actions}
            zones={cfg.zones}
            present={present}
            platform={platform}
            onChange={(combo_actions) => update({ ...cfg, combo_actions })}
            onTry={(key, mods) => api.press(key, mods)}
          />
          <div className="learned">
            <h3>Media keys</h3>
            {Object.keys(cfg.learned_keys).length === 0 ? (
              <p className="special-hint">
                Press an F-key with its function (volume, brightness, playback …). The app then asks which key it belongs to and lights
                up for that function from then on.
              </p>
            ) : (
              <div className="zone-chips">
                {Object.entries(cfg.learned_keys).map(([token, led]) => (
                  <span key={token} className="learned-chip">
                    {tokenLabel(token)} → {led ? keyLabel(led, platform) || "Space" : "ignored"}
                    <button
                      title="Remove assignment"
                      onClick={() => {
                        const { [token]: _removed, ...rest } = cfg.learned_keys;
                        update({ ...cfg, learned_keys: rest });
                      }}
                    >
                      ✕
                    </button>
                  </span>
                ))}
              </div>
            )}
          </div>
        </section>
      </main>
    </div>
  );
}
