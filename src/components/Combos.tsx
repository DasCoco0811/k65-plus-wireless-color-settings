import { useState } from "react";
import { MOD, type Action, type Zone } from "../api";
import { comboLabel, comboName, keyLabel, modLabel, parseCombo } from "../keys";
import { ActionDetails, ActionSelect } from "./ActionPicker";

type Props = {
  combos: Record<string, Action>;
  zones: Zone[];
  present: Set<string>;
  platform: string;
  onChange: (combos: Record<string, Action>) => void;
  onTry: (key: string, mods: number) => void;
};

const MODIFIER_KEYS = new Set(["LeftShift", "RightShift", "LeftCtrl", "RightCtrl", "LeftAlt", "RightAlt", "LeftGui", "RightGui", "Fn"]);
// Order of the modifier buttons when adding a combo.
const MOD_BUTTONS = [MOD.gui, MOD.alt, MOD.ctrl, MOD.shift];

/** What the default combos do on each system. */
function meaning(combo: string, platform: string): string | null {
  const mac = platform === "macos";
  const known: Record<string, string> = mac
    ? { "Gui+Backspace": "Delete line", "Alt+Backspace": "Delete word", "Gui+Space": "Spotlight", "Gui+Tab": "Switch apps" }
    : { "Ctrl+Backspace": "Delete word", "Alt+Tab": "Switch apps", "Gui+Tab": "Task view" };
  return known[combo] ?? null;
}

export function Combos({ combos, zones, present, platform, onChange, onTry }: Props) {
  const [mods, setMods] = useState<number>(MOD.gui);
  const [key, setKey] = useState("Backspace");
  const keys = [...present].filter((k) => !MODIFIER_KEYS.has(k));
  const set = (combo: string, action: Action) => onChange({ ...combos, [combo]: action });
  const remove = (combo: string) => {
    const { [combo]: _removed, ...rest } = combos;
    onChange(rest);
  };
  const newCombo = comboName(mods, key);

  return (
    <div className="combos">
      <div className="special-grid">
        {Object.entries(combos)
          .filter(([combo]) => parseCombo(combo) && present.has(parseCombo(combo)!.key))
          .map(([combo, action]) => {
            const parsed = parseCombo(combo)!;
            const hint = meaning(combo, platform);
            return (
              <div key={combo} className="special-row">
                <div className="special-head">
                  <span className="keycap keycap-combo">{comboLabel(combo, platform)}</span>
                  <span className="special-name">{hint ?? ""}</span>
                  <ActionSelect action={action} zones={zones} onChange={(a) => set(combo, a)} />
                  <button className="btn btn-small" title="Try it on the keyboard" onClick={() => onTry(parsed.key, parsed.mods)}>
                    Try
                  </button>
                  <button className="btn btn-small btn-ghost" title="Remove combo" onClick={() => remove(combo)}>
                    ✕
                  </button>
                </div>
                <ActionDetails action={action} zones={zones} onChange={(a) => set(combo, a)} />
              </div>
            );
          })}
      </div>
      <div className="combo-add">
        <span className="special-hint">New combo:</span>
        {MOD_BUTTONS.map((bit) => (
          <button key={bit} className={`zone-chip ${mods & bit ? "zone-chip-on" : ""}`} onClick={() => setMods(mods ^ bit)}>
            {modLabel(bit, platform)}
          </button>
        ))}
        <span className="special-hint">+</span>
        <select value={key} onChange={(e) => setKey(e.target.value)}>
          {keys.map((k) => (
            <option key={k} value={k}>
              {keyLabel(k, platform) || "Space"} {keyLabel(k, platform) !== k ? `(${k})` : ""}
            </option>
          ))}
        </select>
        <button
          className="btn btn-small btn-accent"
          disabled={mods === 0 || newCombo in combos}
          onClick={() => set(newCombo, { type: "wave" })}
        >
          Add
        </button>
      </div>
    </div>
  );
}
