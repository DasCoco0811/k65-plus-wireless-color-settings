import type { Palette, Zone } from "../api";
import { similarPairs } from "../colors";
import { ColorField, Toggle } from "./Controls";

type Props = {
  zones: Zone[];
  present: Set<string>;
  editZone: number | null;
  globalPalette: Palette;
  ledCorrection: boolean;
  onChange: (zones: Zone[]) => void;
  onEdit: (zone: number | null) => void;
};

export function Zones({ zones, present, editZone, globalPalette, ledCorrection, onChange, onEdit }: Props) {
  const patch = (i: number, p: Partial<Zone>) => onChange(zones.map((z, j) => (j === i ? { ...z, ...p } : z)));

  const add = () => {
    const zone: Zone = {
      id: `zone-${Date.now()}`,
      name: "New zone",
      enabled: true,
      keys: [],
      own_colors: false,
      palette: { ...globalPalette },
    };
    onChange([...zones, zone]);
    onEdit(zones.length);
  };

  const remove = (i: number) => {
    if (editZone !== null) onEdit(null);
    onChange(zones.filter((_, j) => j !== i));
  };

  return (
    <div className="zones">
      {zones.map((z, i) => {
        const count = z.keys.filter((k) => present.has(k)).length;
        const editing = editZone === i;
        return (
          <article key={z.id} className={`zone ${editing ? "zone-editing" : ""} ${z.enabled ? "" : "zone-off"}`}>
            <div className="zone-head">
              <span className="zone-num">{i + 1}</span>
              <input className="zone-name" value={z.name} onChange={(e) => patch(i, { name: e.target.value })} />
              <span className="zone-count">{count === 0 ? "no keys" : `${count} ${count === 1 ? "key" : "keys"}`}</span>
              <button className={`btn btn-small ${editing ? "btn-accent" : ""}`} onClick={() => onEdit(editing ? null : i)}>
                {editing ? "Done" : "Select keys"}
              </button>
              <button className="btn btn-small btn-ghost" title="Delete zone" onClick={() => remove(i)}>
                ✕
              </button>
            </div>
            <div className="zone-options">
              <Toggle label="Active" checked={z.enabled} onChange={(v) => patch(i, { enabled: v })} />
              <Toggle label="Own colors" checked={z.own_colors} onChange={(v) => patch(i, { own_colors: v })} />
            </div>
            {z.own_colors && (
              <div className="zone-colors">
                <ColorField label="Base" value={z.palette.base} onChange={(v) => patch(i, { palette: { ...z.palette, base: v } })} />
                <ColorField label="Ring 1" value={z.palette.primary} onChange={(v) => patch(i, { palette: { ...z.palette, primary: v } })} />
                <ColorField label="Ring 2" value={z.palette.secondary} onChange={(v) => patch(i, { palette: { ...z.palette, secondary: v } })} />
                {similarPairs(z.palette, ledCorrection).map((pair) => (
                  <p key={pair} className="similar">
                    {pair}: hard to tell apart on the keyboard
                  </p>
                ))}
              </div>
            )}
          </article>
        );
      })}
      <button className="btn btn-dashed" onClick={add}>
        + Add zone
      </button>
    </div>
  );
}
