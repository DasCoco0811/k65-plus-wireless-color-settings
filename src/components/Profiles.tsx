import { useState } from "react";
import type { Action, Config, Library, Profile } from "../api";
import { PRESETS } from "../keys";
import { NameForm } from "./NameForm";

type Props = {
  current: Config;
  defaults: Config;
  library: Library;
  /** Loads settings; `id` = one of your profiles, which then auto-saves. */
  onLoad: (config: Config, id: string | null) => void;
  onCreate: (name: string) => void;
  onRename: (id: string, name: string) => void;
  onDuplicate: (name: string, config: Config) => void;
  onDelete: (id: string) => void;
};

/** JSON with sorted keys, so equal settings compare equal. */
function stable(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stable).join(",")}]`;
  if (value && typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>).sort(([a], [b]) => a.localeCompare(b));
    return `{${entries.map(([k, v]) => `${JSON.stringify(k)}:${stable(v)}`).join(",")}}`;
  }
  return JSON.stringify(value);
}

const settings = (c: Config) => stable({ ...c, paused: false });
const palette = (name: string) => PRESETS.find((p) => p.name === name)!.palette;

function templates(d: Config): { name: string; config: Config }[] {
  const typing: Action = { type: "reset_zones", zones: ["letters", "numbers"] };
  return [
    { name: "Default", config: d },
    {
      name: "Calm",
      config: {
        ...d,
        palette: palette("Night"),
        press_fade_ms: 160,
        reset_fade_ms: 2000,
        timeout_secs: 120,
        ambient: { mode: "breathe", speed: 0.6, intensity: 60 },
        key_actions: { Space: typing, Enter: typing, Escape: { type: "reset_all" }, Backspace: { type: "cycle_zones", zones: ["letters", "numbers"] } },
        combo_actions: { "Gui+Backspace": { type: "reset_zones", zones: ["letters", "numbers"] } },
      },
    },
    {
      name: "Party",
      config: {
        ...d,
        palette: palette("Synthwave"),
        reach: 2,
        press_fade_ms: 30,
        reset_fade_ms: 600,
        timeout_secs: 30,
        ambient: { mode: "wave", speed: 1.8, intensity: 100 },
        key_actions: {
          ...d.key_actions,
          LeftShift: { type: "sparkle" },
          RightShift: { type: "sparkle" },
          Delete: { type: "flash" },
          Space: { type: "wave" },
        },
      },
    },
  ];
}

export function Profiles({ current, defaults, library, onLoad, onCreate, onRename, onDuplicate, onDelete }: Props) {
  const [armed, setArmed] = useState<string | null>(null);
  const [editing, setEditing] = useState<{ id: string; name: string } | null>(null);
  const now = settings(current);
  const tpls = templates(defaults);
  const active = library.profiles.find((p) => p.id === library.active_profile) ?? null;
  const template = active ? null : tpls.find((t) => settings(t.config) === now) ?? null;

  const commitRename = () => {
    if (editing && editing.name.trim()) onRename(editing.id, editing.name.trim());
    setEditing(null);
  };

  const savedProfile = (p: Profile) => (
    <span key={p.id} className={`profile ${p.id === active?.id ? "profile-active" : ""}`}>
      {editing?.id === p.id ? (
        <input
          className="profile-rename"
          autoFocus
          value={editing.name}
          onChange={(e) => setEditing({ id: p.id, name: e.target.value })}
          onBlur={commitRename}
          onKeyDown={(e) => {
            if (e.key === "Enter") commitRename();
            if (e.key === "Escape") setEditing(null);
          }}
        />
      ) : (
        <button className="profile-main" onClick={() => onLoad(p.config, p.id)}>
          {p.name}
        </button>
      )}
      <button className="profile-act" title="Rename" onClick={() => setEditing({ id: p.id, name: p.name })}>
        ✎
      </button>
      <button className="profile-act" title="Duplicate" onClick={() => onDuplicate(p.name, p.config)}>
        ⧉
      </button>
      <button
        className={`profile-act ${armed === p.id ? "profile-act-armed" : ""}`}
        title="Delete profile"
        onClick={() => {
          if (armed === p.id) {
            onDelete(p.id);
            setArmed(null);
          } else setArmed(p.id);
        }}
        onBlur={() => setArmed(null)}
      >
        {armed === p.id ? "delete?" : "✕"}
      </button>
    </span>
  );

  return (
    <div className="profiles-wrap">
      <div className="profiles">
        {tpls.map((t) => (
          <span key={t.name} className={`profile ${t === template ? "profile-active" : ""}`}>
            <button className="profile-main" onClick={() => onLoad(t.config, null)}>
              {t.name}
              <span className="profile-tag">Template</span>
            </button>
            <button className="profile-act" title="Duplicate as your own profile" onClick={() => onDuplicate(t.name, t.config)}>
              ⧉
            </button>
          </span>
        ))}
        {library.profiles.map(savedProfile)}
        <NameForm label="Save current settings as a profile" placeholder="Profile name" onSave={onCreate} />
      </div>
      <p className="profile-status">
        {active ? (
          <>
            <b>{active.name}</b> is active. Every change is saved to it automatically.
          </>
        ) : template ? (
          <>
            Template <b>{template.name}</b> loaded. Changes are not saved to the template. Use ⧉ to make it your own profile.
          </>
        ) : (
          <>No profile active. Save the current settings as a profile and changes will be saved to it automatically.</>
        )}
      </p>
    </div>
  );
}
