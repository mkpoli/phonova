//! Spectrogram palette vocabulary shared by the picker, the gradient editor,
//! and the spectrogram pane.
//!
//! A palette is either a built-in ramp (rendered in the engine from an embedded
//! lookup table) or a custom ramp built from color stops in the gradient editor.
//! A custom ramp is expanded to the same 256-entry, 8-bit sRGB lookup table the
//! engine's `colorize_with_lut` samples, so a custom ramp and a built-in one
//! render through one path.

import type { WasmColormapName } from './types';

/** A single color stop on the gradient bar: a position in `[0, 1]` and a color. */
export interface GradientStop {
  /** Position along the ramp, `0` at the display floor and `1` at the ceiling. */
  pos: number;
  /** `#rrggbb` sRGB color. */
  color: string;
}

/** A user-built ramp, persisted across sessions. */
export interface CustomRamp {
  id: string;
  name: string;
  stops: GradientStop[];
}

/** The active spectrogram palette: a built-in ramp or a custom one. */
export type PaletteSelection =
  | { kind: 'builtin'; name: WasmColormapName }
  | { kind: 'custom'; ramp: CustomRamp };

/** The default palette on a fresh load: the brand ramp. */
export const DEFAULT_PALETTE: PaletteSelection = { kind: 'builtin', name: 'Phonia' };

/** Built-in ramp metadata for the picker: display name and preview sample points. */
export interface BuiltinPalette {
  name: WasmColormapName;
  label: string;
  /** A few representative `#rrggbb` sample points, floor to ceiling, for a CSS preview. */
  preview: string[];
  /** One-line note shown in the picker. */
  note?: string;
}

// Sample points are canonical stops of each ramp (matplotlib control points for
// the perceptual ramps; the Phonia anchors for the brand ramp). They drive only
// the small preview strip — the engine renders the real 256-entry table.
export const BUILTIN_PALETTES: BuiltinPalette[] = [
  {
    name: 'Phonia',
    label: 'Phonia',
    preview: ['#17160f', '#194b4e', '#26827a', '#65bda2', '#f7edcb'],
    note: 'Brand default'
  },
  {
    name: 'Viridis',
    label: 'Viridis',
    preview: ['#440154', '#414487', '#2a788e', '#22a884', '#7ad151', '#fde725'],
    note: 'Colorblind-validated'
  },
  {
    name: 'Magma',
    label: 'Magma',
    preview: ['#000004', '#3b0f70', '#8c2981', '#de4968', '#fe9f6d', '#fcfdbf']
  },
  {
    name: 'Inferno',
    label: 'Inferno',
    preview: ['#000004', '#420a68', '#932667', '#dd513a', '#fca50a', '#fcffa4']
  },
  {
    name: 'Plasma',
    label: 'Plasma',
    preview: ['#0d0887', '#6a00a8', '#b12a90', '#e16462', '#fca636', '#f0f921']
  },
  {
    name: 'Cividis',
    label: 'Cividis',
    preview: ['#00224e', '#35456c', '#666970', '#97925b', '#cbba69', '#fee838']
  },
  {
    name: 'Golden',
    label: 'Golden',
    preview: ['#17160f', '#482a15', '#6b3f1f', '#c9862f', '#f5d68a'],
    note: 'Warm sibling of Phonia'
  },
  {
    name: 'Grayscale',
    label: 'Grayscale',
    preview: ['#1e1e1e', '#5a5a5a', '#969696', '#c8c8c8', '#ebebeb'],
    note: 'Print'
  }
];

function clamp255(v: number): number {
  return v < 0 ? 0 : v > 255 ? 255 : Math.round(v);
}

/** Parse `#rgb`/`#rrggbb` into an `[r, g, b]` triple of 0–255 integers. */
export function hexToRgb(hex: string): [number, number, number] {
  let h = hex.trim().replace(/^#/, '');
  if (h.length === 3) h = h[0] + h[0] + h[1] + h[1] + h[2] + h[2];
  const n = Number.parseInt(h, 16);
  if (h.length !== 6 || Number.isNaN(n)) return [0, 0, 0];
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

/** Format an `[r, g, b]` triple of 0–255 integers as `#rrggbb`. */
export function rgbToHex(rgb: [number, number, number]): string {
  return '#' + rgb.map((c) => clamp255(c).toString(16).padStart(2, '0')).join('');
}

/** Sorted stops with the endpoints pinned to `0` and `1`, so a ramp always fills. */
function normalizedStops(stops: GradientStop[]): GradientStop[] {
  const sorted = [...stops].sort((a, b) => a.pos - b.pos);
  if (sorted.length === 0) return [{ pos: 0, color: '#000000' }, { pos: 1, color: '#ffffff' }];
  if (sorted.length === 1) return [{ ...sorted[0], pos: 0 }, { ...sorted[0], pos: 1 }];
  return sorted;
}

/**
 * Expand color stops to a 768-byte lookup table: 256 `R, G, B` triples, floor to
 * ceiling, interpolated linearly in 8-bit sRGB between adjacent stops — the same
 * space the engine's LUT sampler interpolates in.
 */
export function rampToLut(stops: GradientStop[]): Uint8Array {
  const s = normalizedStops(stops);
  const out = new Uint8Array(768);
  for (let i = 0; i < 256; i += 1) {
    const t = i / 255;
    let lo = s[0];
    let hi = s[s.length - 1];
    for (let k = 0; k < s.length - 1; k += 1) {
      if (t >= s[k].pos && t <= s[k + 1].pos) {
        lo = s[k];
        hi = s[k + 1];
        break;
      }
    }
    const span = hi.pos - lo.pos;
    const f = span > 0 ? (t - lo.pos) / span : 0;
    const a = hexToRgb(lo.color);
    const b = hexToRgb(hi.color);
    out[i * 3] = clamp255(a[0] + (b[0] - a[0]) * f);
    out[i * 3 + 1] = clamp255(a[1] + (b[1] - a[1]) * f);
    out[i * 3 + 2] = clamp255(a[2] + (b[2] - a[2]) * f);
  }
  return out;
}

/** A `linear-gradient(...)` value for a CSS preview strip built from stops. */
export function rampGradientCss(stops: GradientStop[]): string {
  const s = normalizedStops(stops);
  const parts = s.map((stop) => `${stop.color} ${(stop.pos * 100).toFixed(1)}%`);
  return `linear-gradient(to right, ${parts.join(', ')})`;
}

/** A `linear-gradient(...)` value for a built-in ramp's preview sample points. */
export function builtinGradientCss(preview: string[]): string {
  const parts = preview.map(
    (color, i) => `${color} ${((i / Math.max(1, preview.length - 1)) * 100).toFixed(1)}%`
  );
  return `linear-gradient(to right, ${parts.join(', ')})`;
}

/** The CSS preview gradient for any palette selection. */
export function paletteGradientCss(sel: PaletteSelection): string {
  if (sel.kind === 'custom') return rampGradientCss(sel.ramp.stops);
  const meta = BUILTIN_PALETTES.find((p) => p.name === sel.name);
  return meta ? builtinGradientCss(meta.preview) : 'linear-gradient(to right, #000, #fff)';
}

/** The human label for any palette selection. */
export function paletteLabel(sel: PaletteSelection): string {
  if (sel.kind === 'custom') return sel.ramp.name;
  return BUILTIN_PALETTES.find((p) => p.name === sel.name)?.label ?? sel.name;
}

/** A stable identity string for a palette selection, for tile-cache keys. */
export function paletteKey(sel: PaletteSelection): string {
  if (sel.kind === 'builtin') return `b:${sel.name}`;
  // A custom ramp keys on its id and a hash of its stops, so an edit recolors.
  return `c:${sel.ramp.id}:${rampHash(sel.ramp.stops)}`;
}

function rampHash(stops: GradientStop[]): string {
  let h = 0x811c9dc5;
  const str = stops.map((s) => `${s.pos.toFixed(4)}${s.color}`).join('|');
  for (let i = 0; i < str.length; i += 1) {
    h ^= str.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return (h >>> 0).toString(16);
}

/** WCAG 2.x relative luminance of an 8-bit sRGB triple. */
function relativeLuminance(r: number, g: number, b: number): number {
  const chan = (c: number) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * chan(r) + 0.7152 * chan(g) + 0.0722 * chan(b);
}

/**
 * Whether a ramp's relative luminance never decreases from floor to ceiling —
 * the property the built-in ramps guarantee. A custom ramp that fails this reads
 * a louder region as darker than a quieter one somewhere along the scale.
 */
export function rampIsMonotonic(stops: GradientStop[]): boolean {
  const lut = rampToLut(stops);
  let prev = -1;
  for (let i = 0; i < 256; i += 1) {
    const lum = relativeLuminance(lut[i * 3], lut[i * 3 + 1], lut[i * 3 + 2]);
    if (lum < prev - 1e-6) return false;
    prev = lum;
  }
  return true;
}

// --- Persistence: custom ramps live in localStorage, app-wide (not per project). ---

const STORAGE_KEY = 'phonia:custom-ramps';

/** Read the saved custom ramps, or an empty list when none or unreadable. */
export function loadCustomRamps(): CustomRamp[] {
  if (typeof localStorage === 'undefined') return [];
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (r): r is CustomRamp =>
        r &&
        typeof r.id === 'string' &&
        typeof r.name === 'string' &&
        Array.isArray(r.stops) &&
        r.stops.every(
          (s: unknown) =>
            s &&
            typeof (s as GradientStop).pos === 'number' &&
            typeof (s as GradientStop).color === 'string'
        )
    );
  } catch {
    return [];
  }
}

/** Persist the custom ramps. */
export function saveCustomRamps(ramps: CustomRamp[]): void {
  if (typeof localStorage === 'undefined') return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(ramps));
  } catch {
    // Storage full or unavailable: the ramp stays live for the session.
  }
}

/** A fresh ramp seeded from the brand anchors, ready for editing. */
export function newRampTemplate(): CustomRamp {
  return {
    id: typeof crypto !== 'undefined' && crypto.randomUUID ? crypto.randomUUID() : `ramp-${Date.now()}`,
    name: 'Custom ramp',
    stops: [
      { pos: 0, color: '#17160f' },
      { pos: 0.5, color: '#26827a' },
      { pos: 1, color: '#f7edcb' }
    ]
  };
}
