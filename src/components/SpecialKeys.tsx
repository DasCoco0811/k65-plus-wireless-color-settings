import type { Action, Zone } from "../api";
import { keyLabel } from "../keys";
import { ActionDetails, ActionSelect } from "./ActionPicker";

type Props = {
  actions: Record<string, Action>;
  zones: Zone[];
  present: Set<string>;
  platform: string;
  onChange: (actions: Record<string, Action>) => void;
  onTry: (key: string) => void;
};

const KEYS = [
  "Escape",
  "Tab",
  "CapsLock",
  "LeftShift",
  "RightShift",
  "LeftCtrl",
  "LeftGui",
  "LeftAlt",
  "Space",
  "RightAlt",
  "RightGui",
  "RightCtrl",
  "Enter",
  "Backspace",
  "Delete",
];

function keyName(name: string, platform: string) {
  const mac = platform === "macos";
  const names: Record<string, string> = {
    Escape: "Esc",
    Tab: "Tab",
    CapsLock: "Caps Lock",
    LeftShift: "Left Shift",
    RightShift: "Right Shift",
    LeftCtrl: "Left Ctrl",
    RightCtrl: "Right Ctrl",
    LeftGui: mac ? "Left Cmd" : "Windows key",
    RightGui: mac ? "Right Cmd" : "Right Windows key",
    LeftAlt: mac ? "Left Option" : "Alt",
    RightAlt: mac ? "Right Option" : "AltGr / Right Alt",
    Space: "Space",
    Enter: "Enter",
    Backspace: "Backspace",
    Delete: "Delete",
  };
  return names[name] ?? name;
}

export function SpecialKeys({ actions, zones, present, platform, onChange, onTry }: Props) {
  const set = (key: string, action: Action) => onChange({ ...actions, [key]: action });

  return (
    <div className="special-grid">
      {KEYS.filter((k) => present.has(k)).map((key) => {
        const action: Action = actions[key] ?? { type: "ring" };
        const ownZone = zones.find((z) => z.keys.includes(key))?.id;
        return (
          <div key={key} className="special-row">
            <div className="special-head">
              <span className="keycap">{keyLabel(key, platform) || "␣"}</span>
              <span className="special-name">{keyName(key, platform)}</span>
              <ActionSelect action={action} zones={zones} ownZone={ownZone} onChange={(a) => set(key, a)} />
              <button className="btn btn-small" title="Try it on the keyboard" onClick={() => onTry(key)}>
                Try
              </button>
            </div>
            <ActionDetails action={action} zones={zones} onChange={(a) => set(key, a)} />
          </div>
        );
      })}
    </div>
  );
}
