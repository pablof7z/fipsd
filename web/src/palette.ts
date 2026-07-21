/** Color helpers. Roots get distinct, stable hues; nodes are tinted by their
 * currently-accepted root. Kept dependency-free (no d3-scale) on purpose. */

import type { RootId } from "./types.ts";

// A curated categorical ramp — legible on the dark stage, distinct in hue.
const ROOT_HUES = [199, 152, 45, 280, 340, 20, 95, 230];

export interface Rgb {
  r: number;
  g: number;
  b: number;
}

function hslToRgb(h: number, s: number, l: number): Rgb {
  h /= 360;
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  const hk = (t: number) => {
    if (t < 0) t += 1;
    if (t > 1) t -= 1;
    if (t < 1 / 6) return p + (q - p) * 6 * t;
    if (t < 1 / 2) return q;
    if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
    return p;
  };
  return {
    r: Math.round(hk(h + 1 / 3) * 255),
    g: Math.round(hk(h) * 255),
    b: Math.round(hk(h - 1 / 3) * 255),
  };
}

export function rgbStr({ r, g, b }: Rgb, a = 1): string {
  return a >= 1 ? `rgb(${r},${g},${b})` : `rgba(${r},${g},${b},${a})`;
}

export class RootPalette {
  private index = new Map<RootId, number>();

  hueFor(root: RootId): number {
    if (!this.index.has(root)) this.index.set(root, this.index.size);
    return ROOT_HUES[this.index.get(root)! % ROOT_HUES.length];
  }

  /** Base fill for a node accepting `root`, dimmed by `settle` (0=fresh,1=settled). */
  nodeColor(root: RootId | null, settle: number): Rgb {
    if (root === null) return { r: 90, g: 100, b: 116 };
    const h = this.hueFor(root);
    // Fresh adopters are brighter/more saturated; settled ones calm down.
    const s = 0.55 + 0.35 * (1 - settle);
    const l = 0.5 + 0.18 * (1 - settle);
    return hslToRgb(h, s, l);
  }

  rootColor(root: RootId): Rgb {
    return hslToRgb(this.hueFor(root), 0.7, 0.6);
  }

  /** Known roots in first-seen order (for the legend). */
  entries(): RootId[] {
    return [...this.index.keys()];
  }
}
