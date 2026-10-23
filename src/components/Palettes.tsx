import { useState } from "react";
import type { NamedPalette, Palette } from "../api";
import { PRESETS } from "../keys";
import { NameForm } from "./NameForm";

type Props = {
  current: Palette;
  custom: NamedPalette[];
  onPick: (p: Palette) => void;
  onSaveCustom: (name: string) => void;
  onDeleteCustom: (index: number) => void;
};

const same = (a: Palette, b: Palette) =>
  a.base.toUpperCase() === b.base.toUpperCase() &&
  a.primary.toUpperCase() === b.primary.toUpperCase() &&
  a.secondary.toUpperCase() === b.secondary.toUpperCase();

function Dots({ p }: { p: Palette }) {
  return (
    <span className="preset-dots">
      <i style={{ background: p.base }} />
      <i style={{ background: p.primary }} />
      <i style={{ background: p.secondary }} />
    </span>
  );
}

export function Palettes({ current, custom, onPick, onSaveCustom, onDeleteCustom }: Props) {
  const [armed, setArmed] = useState<number | null>(null);
  return (
    <div className="palettes">
      <div className="presets">
        {PRESETS.map((p) => (
          <button key={p.name} className={`preset ${same(p.palette, current) ? "preset-active" : ""}`} onClick={() => onPick(p.palette)}>
            <Dots p={p.palette} />
            {p.name}
          </button>
        ))}
      </div>
      <h3 className="sub-head">Your sets</h3>
      <div className="presets">
        {custom.map((c, i) => (
          <span key={`${c.name}-${i}`} className={`preset preset-custom ${same(c.palette, current) ? "preset-active" : ""}`}>
            <button className="preset-main" onClick={() => onPick(c.palette)}>
              <Dots p={c.palette} />
              {c.name}
            </button>
            <button
              className={`preset-x ${armed === i ? "preset-x-armed" : ""}`}
              title="Delete set"
              onClick={() => {
                if (armed === i) {
                  onDeleteCustom(i);
                  setArmed(null);
                } else setArmed(i);
              }}
              onBlur={() => setArmed(null)}
            >
              {armed === i ? "delete?" : "✕"}
            </button>
          </span>
        ))}
        <NameForm label="Save current colors as a set" placeholder="Set name" onSave={onSaveCustom} />
      </div>
    </div>
  );
}
