import { useMemo } from "react";
import type { Led } from "../api";
import { keyLabel, luminance, rgb } from "../keys";

type Props = {
  leds: Led[];
  colors: [number, number, number][];
  platform: string;
  /** Zone whose keys are being selected (`null` = preview/test). */
  editZone: number | null;
  zoneOfKey: Map<string, number>;
  lastKey: string | null;
  onKey: (name: string) => void;
};

const GAP = 1.6;
const PAD = 6;

export function Keyboard({ leds, colors, platform, editZone, zoneOfKey, lastKey, onKey }: Props) {
  const box = useMemo(() => {
    if (leds.length === 0) return { x: 0, y: 0, w: 100, h: 40 };
    const x0 = Math.min(...leds.map((l) => l.x - l.w / 2));
    const y0 = Math.min(...leds.map((l) => l.y - l.h / 2));
    const x1 = Math.max(...leds.map((l) => l.x + l.w / 2));
    const y1 = Math.max(...leds.map((l) => l.y + l.h / 2));
    return { x: x0 - PAD, y: y0 - PAD, w: x1 - x0 + 2 * PAD, h: y1 - y0 + 2 * PAD };
  }, [leds]);

  const editing = editZone !== null;

  return (
    <svg className={`kb ${editing ? "kb-editing" : ""}`} viewBox={`${box.x} ${box.y} ${box.w} ${box.h}`} role="img" aria-label="Keyboard preview">
      <defs>
        <filter id="glow" x="-30%" y="-30%" width="160%" height="160%">
          <feGaussianBlur stdDeviation="3.4" />
        </filter>
        <linearGradient id="cap" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#fff" stopOpacity="0.22" />
          <stop offset="0.45" stopColor="#fff" stopOpacity="0" />
          <stop offset="1" stopColor="#000" stopOpacity="0.28" />
        </linearGradient>
      </defs>
      <rect className="kb-case" x={box.x + 1} y={box.y + 1} width={box.w - 2} height={box.h - 2} rx="7" />
      {!editing && (
        <g filter="url(#glow)" opacity="0.9">
          {leds.map((l, i) => (
            <rect key={l.name} x={l.x - l.w / 2} y={l.y - l.h / 2} width={l.w} height={l.h} rx="3" fill={rgb(colors[i] ?? [0, 0, 0])} />
          ))}
        </g>
      )}
      {leds.map((l, i) => {
        const color = colors[i] ?? [0, 0, 0];
        const zone = zoneOfKey.get(l.name);
        const inEdit = editing && zone === editZone;
        const otherZone = editing && zone !== undefined && zone !== editZone;
        const fill = !editing ? rgb(color) : inEdit ? rgb(color) : "#231d2d";
        const textDark = !editing || inEdit ? luminance(color) > 0.55 : false;
        const label = keyLabel(l.name, platform);
        return (
          <g
            key={l.name}
            className={`key ${inEdit ? "key-in" : ""} ${otherZone ? "key-other" : ""} ${lastKey === l.name ? "key-last" : ""}`}
            onClick={() => onKey(l.name)}
          >
            <rect x={l.x - l.w / 2 + GAP / 2} y={l.y - l.h / 2 + GAP / 2} width={l.w - GAP} height={l.h - GAP} rx="2.6" fill={fill} />
            <rect className="key-shade" x={l.x - l.w / 2 + GAP / 2} y={l.y - l.h / 2 + GAP / 2} width={l.w - GAP} height={l.h - GAP} rx="2.6" fill="url(#cap)" />
            {label && (
              <text x={l.x} y={l.y} fontSize={label.length > 3 ? 3.6 : 5} fill={textDark ? "#1a0f24" : "#fff"}>
                {label}
              </text>
            )}
            {otherZone && (
              <text className="zone-tag" x={l.x + l.w / 2 - 3.2} y={l.y - l.h / 2 + 4.2}>
                {(zone ?? 0) + 1}
              </text>
            )}
          </g>
        );
      })}
    </svg>
  );
}
