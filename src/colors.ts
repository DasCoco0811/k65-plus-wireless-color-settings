import type { Palette } from "./api";

/** Below this OKLab distance two tones blur together on the keyboard. */
const TOO_SIMILAR = 0.16;

const toLinear = (c: number) => {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
};

const toSrgb = (l: number) => Math.round(255 * (l <= 0.0031308 ? 12.92 * l : 1.055 * l ** (1 / 2.4) - 0.055));

const channels = (hex: string) => {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
};

function oklab([r8, g8, b8]: number[]): [number, number, number] {
  const [r, g, b] = [r8, g8, b8].map(toLinear);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

/**
 * How the keyboard shows a color. With correction the LED light matches the
 * screen color; without it the raw values are emitted as linear light and
 * look brighter and paler.
 */
function onKeyboard(hex: string, ledCorrection: boolean): number[] {
  const c = channels(hex);
  return ledCorrection ? c : c.map((v) => toSrgb(v / 255));
}

function distance(a: string, b: string, ledCorrection: boolean) {
  const [x, y] = [oklab(onKeyboard(a, ledCorrection)), oklab(onKeyboard(b, ledCorrection))];
  return Math.hypot(x[0] - y[0], x[1] - y[1], x[2] - y[2]);
}

/** Color pairs that are hard to tell apart on the keyboard. */
export function similarPairs(p: Palette, ledCorrection: boolean): string[] {
  const pairs: [string, string, string][] = [
    [p.base, p.primary, "Base color and ring 1"],
    [p.primary, p.secondary, "Ring 1 and ring 2"],
    [p.base, p.secondary, "Base color and ring 2"],
  ];
  return pairs.filter(([a, b]) => distance(a, b, ledCorrection) < TOO_SIMILAR).map(([, , label]) => label);
}
