import type { Action, Zone } from "../api";

export const ACTIONS: { type: Action["type"]; label: string; hint: string }[] = [
  { type: "ring", label: "Normal (ring)", hint: "Like any other key" },
  { type: "reset_zones", label: "Reset zones", hint: "Selected zones back to the base color" },
  { type: "reset_all", label: "Reset everything", hint: "All zones back to the base color" },
  { type: "cycle_zones", label: "Cycle colors", hint: "Every key moves one color on: base → ring 1 → ring 2" },
  { type: "wipe", label: "Wipe", hint: "A ring 1 stripe wipes from right to left and erases the selected zones as it passes" },
  { type: "wave", label: "Wave", hint: "Ring 1 spreads from the key across the keyboard" },
  { type: "sweep", label: "Sweep", hint: "A ring 2 stripe runs from left to right" },
  { type: "flash", label: "Flash", hint: "The whole keyboard flashes in ring 2" },
  { type: "sparkle", label: "Sparkle", hint: "Random keys sparkle briefly" },
  { type: "rise", label: "Rise", hint: "A stripe rises from the bottom row to the top, turning from ring 1 to ring 2" },
];

type ZoneAction = Extract<Action, { zones: string[] }>;
const hasZones = (a: Action): a is ZoneAction => "zones" in a;

function withType(type: Action["type"], current: Action, zones: Zone[], ownZone: string | undefined): Action {
  if (type !== "reset_zones" && type !== "cycle_zones" && type !== "wipe") return { type };
  if (hasZones(current)) return { type, zones: current.zones };
  const fallback = type === "reset_zones" ? [ownZone ?? "letters"] : type === "cycle_zones" ? ["letters", "numbers"] : [];
  return { type, zones: fallback.filter((id) => zones.some((z) => z.id === id)) };
}

type Props = {
  action: Action;
  zones: Zone[];
  /** Zone of the key, suggested for "Reset zones". */
  ownZone?: string;
  onChange: (action: Action) => void;
};

/** Picks the action plus, where needed, the zones it affects. */
export function ActionSelect({ action, zones, ownZone, onChange }: Props) {
  return (
    <select value={action.type} onChange={(e) => onChange(withType(e.target.value as Action["type"], action, zones, ownZone))}>
      {ACTIONS.map((a) => (
        <option key={a.type} value={a.type}>
          {a.label}
        </option>
      ))}
    </select>
  );
}

export function ActionDetails({ action, zones, onChange }: Omit<Props, "ownZone">) {
  if (!hasZones(action)) {
    return <span className="special-hint">{ACTIONS.find((a) => a.type === action.type)?.hint}</span>;
  }
  return (
    <div className="zone-chips">
      {zones.map((z) => {
        const on = action.zones.includes(z.id);
        return (
          <button
            key={z.id}
            className={`zone-chip ${on ? "zone-chip-on" : ""}`}
            onClick={() => onChange({ ...action, zones: on ? action.zones.filter((id) => id !== z.id) : [...action.zones, z.id] })}
          >
            {z.name}
          </button>
        );
      })}
      {action.type === "wipe" && action.zones.length === 0 && <span className="special-hint">no zone = effect only</span>}
    </div>
  );
}
