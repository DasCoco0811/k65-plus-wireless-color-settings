import type { CSSProperties } from "react";
import type { Ambient, AmbientMode, Palette } from "../api";
import { Slider } from "./Controls";

const MODES: { mode: AmbientMode; label: string; hint: string }[] = [
  { mode: "off", label: "Off", hint: "Static base color" },
  { mode: "aurora", label: "Waves", hint: "Two color waves slowly flow into each other" },
  { mode: "wave", label: "Color wave", hint: "A color gradient travels across the keyboard" },
  { mode: "fade", label: "Color fade", hint: "The whole keyboard slowly fades through the three colors" },
  { mode: "breathe", label: "Breathe", hint: "Gentle dimming up and down" },
  { mode: "glimmer", label: "Glimmer", hint: "Single keys softly glow up and fade out" },
];

type Props = {
  ambient: Ambient;
  palette: Palette;
  onChange: (ambient: Ambient) => void;
  onShow: () => void;
};

export function AmbientPicker({ ambient, palette, onChange, onShow }: Props) {
  const vars = {
    "--c0": palette.base,
    "--c1": palette.primary,
    "--c2": palette.secondary,
    "--speed": ambient.speed,
  } as CSSProperties;
  const current = MODES.find((m) => m.mode === ambient.mode);

  return (
    <div className="ambient">
      <div className="ambient-modes" style={vars}>
        {MODES.map((m) => (
          <button
            key={m.mode}
            className={`ambient-mode ${m.mode === ambient.mode ? "ambient-mode-active" : ""}`}
            onClick={() => onChange({ ...ambient, mode: m.mode })}
            title={m.hint}
          >
            <span className={`ambient-preview amb-${m.mode}`} aria-hidden />
            <span className="ambient-label">{m.label}</span>
          </button>
        ))}
      </div>
      <p className="special-hint">{current?.hint}</p>
      {ambient.mode !== "off" && (
        <div className="ambient-sliders">
          <Slider
            label="Speed"
            value={ambient.speed}
            min={0.25}
            max={3}
            step={0.05}
            format={(v) => `${v.toFixed(2).replace(/\.?0+$/, "")}×`}
            onChange={(speed) => onChange({ ...ambient, speed })}
          />
          <Slider
            label="Strength"
            value={ambient.intensity}
            min={10}
            max={100}
            format={(v) => `${v} %`}
            onChange={(intensity) => onChange({ ...ambient, intensity })}
          />
        </div>
      )}
      <div className="profile-io">
        <button className="btn btn-small" onClick={onShow}>
          Reset keyboard and watch
        </button>
        <span className="special-hint">The background only plays while no key is colored.</span>
      </div>
    </div>
  );
}
