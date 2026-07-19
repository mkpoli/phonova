// Synthesized spectrogram engine for the landing page. Generates a
// source-filter speech-like signal from formant keyframes and renders it
// through the Phonia-style warm charcoal → amber ramp. Data art, not a
// measurement: every panel that uses it is labeled as synthesized.

export interface Key {
  t: number;
  F: [number, number, number, number];
  g?: [number, number, number, number];
  amp: number;
  f0: number;
  n: { g: number; lo: number; hi: number };
}

export interface Spec {
  norm: Float32Array;
  cols: number;
  rows: number;
  maxF: number;
  dur: number;
  f0Arr: Float32Array;
  ampArr: Float32Array;
  form: Float32Array[];
}

const STOPS = [
  [0, 26, 23, 19],
  [0.22, 54, 41, 27],
  [0.42, 105, 73, 37],
  [0.62, 168, 117, 48],
  [0.8, 215, 162, 68],
  [0.92, 246, 198, 112],
  [1, 253, 232, 172]
];

const LUT = (() => {
  const a = new Uint8Array(256 * 3);
  for (let i = 0; i < 256; i++) {
    const t = i / 255;
    let s = 0;
    while (s < STOPS.length - 2 && t > STOPS[s + 1][0]) s++;
    const A = STOPS[s];
    const B = STOPS[s + 1];
    const u = (t - A[0]) / (B[0] - A[0] || 1);
    a[i * 3] = A[1] + (B[1] - A[1]) * u;
    a[i * 3 + 1] = A[2] + (B[2] - A[2]) * u;
    a[i * 3 + 2] = A[3] + (B[3] - A[3]) * u;
  }
  return a;
})();

const lerp = (a: number, b: number, u: number) => a + (b - a) * u;
const sstep = (u: number) => {
  u = Math.min(1, Math.max(0, u));
  return u * u * u * (u * (u * 6 - 15) + 10);
};

export function sampleKeys(keys: Key[], t: number) {
  let i = 0;
  while (i < keys.length - 2 && t > keys[i + 1].t) i++;
  const k0 = keys[i];
  const k1 = keys[i + 1];
  const u = sstep((t - k0.t) / Math.max(1e-6, k1.t - k0.t));
  return {
    F: [0, 1, 2, 3].map((j) => lerp(k0.F[j], k1.F[j], u)),
    g: [0, 1, 2, 3].map((j) => lerp(k0.g ? k0.g[j] : 1, k1.g ? k1.g[j] : 1, u)),
    amp: lerp(k0.amp, k1.amp, u),
    f0: k0.f0 > 0 && k1.f0 > 0 ? lerp(k0.f0, k1.f0, u) : 0,
    ng: lerp(k0.n.g, k1.n.g, u),
    nlo: lerp(k0.n.lo, k1.n.lo, u),
    nhi: lerp(k0.n.hi, k1.n.hi, u)
  };
}

export function buildSpec(cfg: {
  cols: number;
  rows: number;
  maxF: number;
  dur: number;
  keys: Key[];
}): Spec {
  const { cols, rows, maxF, dur, keys } = cfg;
  const grid = new Float32Array(cols * rows);
  const f0Arr = new Float32Array(cols);
  const ampArr = new Float32Array(cols);
  const form = [new Float32Array(cols), new Float32Array(cols), new Float32Array(cols)];
  const B = [90, 110, 150, 210];
  for (let c = 0; c < cols; c++) {
    const t = ((c + 0.5) / cols) * dur;
    const p = sampleKeys(keys, t);
    let f0 = p.f0;
    if (f0 > 0) f0 *= 1 + 0.012 * Math.sin(2 * Math.PI * 5.2 * t);
    f0Arr[c] = f0;
    ampArr[c] = p.amp;
    form[0][c] = p.F[0];
    form[1][c] = p.F[1];
    form[2][c] = p.F[2];
    const base = c * rows;
    if (f0 > 0 && p.amp > 0.004) {
      for (let h = 1; ; h++) {
        const f = h * f0;
        if (f > maxF) break;
        let w = 0;
        for (let k = 0; k < 4; k++) {
          const s = B[k] / 2;
          const d = (f - p.F[k]) / s;
          w += p.g[k] * Math.exp(-0.5 * d * d);
        }
        let A = p.amp * Math.pow(h, -1.4) * (0.02 + w);
        A *= 0.9 + 0.2 * Math.random();
        A *= 0.7 + 0.3 * Math.cos(2 * Math.PI * f0 * t);
        const rf = (1 - f / maxF) * (rows - 1);
        const r0 = Math.round(rf);
        const spread = Math.max(1.3, (0.6 * f0 * rows) / maxF);
        const ext = Math.ceil(spread * 2.5);
        for (let dr = -ext; dr <= ext; dr++) {
          const r = r0 + dr;
          if (r < 0 || r >= rows) continue;
          const d = (rf - r) / spread;
          grid[base + r] += A * Math.exp(-0.5 * d * d);
        }
      }
      for (let r = 0; r < rows; r++) {
        if (Math.random() < 0.22) grid[base + r] += p.amp * 0.005 * Math.random();
      }
    }
    if (p.ng > 0 && p.nhi > p.nlo) {
      const nc = (p.nlo + p.nhi) / 2;
      const nw = Math.max(120, (p.nhi - p.nlo) / 2);
      for (let r = 0; r < rows; r++) {
        const f = maxF * (1 - r / (rows - 1));
        const d = (f - nc) / nw;
        grid[base + r] += p.ng * Math.exp(-0.5 * d * d) * (0.25 + 0.75 * Math.random());
      }
    }
  }
  let max = 1e-9;
  for (let i = 0; i < grid.length; i++) if (grid[i] > max) max = grid[i];
  const norm = new Float32Array(grid.length);
  for (let i = 0; i < grid.length; i++) {
    const db = 20 * Math.log10(grid[i] / max + 1e-6);
    let v = (db + 55) / 55;
    if (v < 0) v = 0;
    if (v > 1) v = 1;
    norm[i] = Math.pow(v, 1.12);
  }
  return { norm, cols, rows, maxF, dur, f0Arr, ampArr, form };
}

export function specToCanvas(spec: Spec): HTMLCanvasElement {
  const cv = document.createElement('canvas');
  cv.width = spec.cols;
  cv.height = spec.rows;
  const ctx = cv.getContext('2d')!;
  const img = ctx.createImageData(spec.cols, spec.rows);
  const d = img.data;
  for (let c = 0; c < spec.cols; c++) {
    for (let r = 0; r < spec.rows; r++) {
      const v = Math.min(255, Math.round(spec.norm[c * spec.rows + r] * 255));
      const i = (r * spec.cols + c) * 4;
      d[i] = LUT[v * 3];
      d[i + 1] = LUT[v * 3 + 1];
      d[i + 2] = LUT[v * 3 + 2];
      d[i + 3] = 255;
    }
  }
  ctx.putImageData(img, 0, 0);
  return cv;
}

export interface PlotBox {
  l: number;
  t: number;
  w: number;
  h: number;
}

function trackPath(
  ctx: CanvasRenderingContext2D,
  spec: Spec,
  plot: PlotBox,
  arr: Float32Array,
  breakPred?: (c: number) => boolean
) {
  let pen = false;
  ctx.beginPath();
  for (let c = 0; c < spec.cols; c++) {
    const f = arr[c];
    const ok = f > 0 && (!breakPred || breakPred(c));
    if (!ok) {
      pen = false;
      continue;
    }
    const x = plot.l + (c / (spec.cols - 1)) * plot.w;
    const y = plot.t + (1 - f / spec.maxF) * plot.h;
    if (!pen) {
      ctx.moveTo(x, y);
      pen = true;
    } else ctx.lineTo(x, y);
  }
  ctx.stroke();
}

export function strokeTrack(
  ctx: CanvasRenderingContext2D,
  spec: Spec,
  plot: PlotBox,
  arr: Float32Array,
  color: string,
  width: number,
  breakPred?: (c: number) => boolean
) {
  ctx.save();
  ctx.strokeStyle = color;
  ctx.lineWidth = width;
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';
  ctx.shadowColor = color;
  ctx.shadowBlur = 7;
  ctx.globalAlpha = 0.95;
  trackPath(ctx, spec, plot, arr, breakPred);
  ctx.restore();
}

export const HERO_KEYS: Key[] = [
  { t: 0.0, F: [500, 1500, 2500, 3300], amp: 0.0, f0: 0, n: { g: 0.004, lo: 200, hi: 5200 } },
  { t: 0.3, F: [730, 1090, 2440, 3300], amp: 0.9, f0: 116, n: { g: 0.002, lo: 200, hi: 5200 } },
  { t: 0.64, F: [740, 1120, 2460, 3300], amp: 0.95, f0: 124, n: { g: 0.002, lo: 200, hi: 5200 } },
  { t: 0.98, F: [270, 2290, 3010, 3500], amp: 0.8, f0: 128, n: { g: 0.002, lo: 200, hi: 5200 } },
  { t: 1.12, F: [270, 2290, 3010, 3500], amp: 0.22, f0: 0, n: { g: 0.34, lo: 4000, hi: 5500 } },
  { t: 1.45, F: [300, 1400, 2600, 3400], amp: 0.2, f0: 0, n: { g: 0.3, lo: 4200, hi: 5500 } },
  { t: 1.58, F: [400, 1200, 2500, 3300], amp: 0.02, f0: 0, n: { g: 0.006, lo: 200, hi: 5200 } },
  { t: 1.74, F: [300, 870, 2240, 3000], amp: 0.85, f0: 110, n: { g: 0.002, lo: 200, hi: 5200 } },
  { t: 2.02, F: [305, 880, 2260, 3000], amp: 0.88, f0: 117, n: { g: 0.002, lo: 200, hi: 5200 } },
  {
    t: 2.14,
    F: [255, 1000, 2200, 2900],
    g: [1, 0.3, 0.12, 0.06],
    amp: 0.5,
    f0: 107,
    n: { g: 0.003, lo: 200, hi: 5200 }
  },
  {
    t: 2.34,
    F: [260, 1000, 2200, 2900],
    g: [1, 0.3, 0.12, 0.06],
    amp: 0.45,
    f0: 105,
    n: { g: 0.003, lo: 200, hi: 5200 }
  },
  { t: 2.5, F: [750, 1150, 2500, 3300], amp: 0.92, f0: 112, n: { g: 0.002, lo: 200, hi: 5200 } },
  { t: 2.95, F: [480, 1750, 2750, 3400], amp: 0.85, f0: 123, n: { g: 0.002, lo: 200, hi: 5200 } },
  { t: 3.32, F: [310, 2050, 2900, 3450], amp: 0.55, f0: 97, n: { g: 0.004, lo: 200, hi: 5200 } },
  { t: 3.75, F: [350, 1500, 2600, 3400], amp: 0.12, f0: 0, n: { g: 0.012, lo: 200, hi: 5200 } },
  { t: 4.2, F: [500, 1500, 2500, 3300], amp: 0.0, f0: 0, n: { g: 0.004, lo: 200, hi: 5200 } }
];

export const MINI_KEYS: Key[] = [
  { t: 0, F: [500, 1500, 2500, 3300], amp: 0, f0: 0, n: { g: 0.004, lo: 300, hi: 4800 } },
  { t: 0.18, F: [280, 2250, 3000, 3500], amp: 0.8, f0: 130, n: { g: 0.002, lo: 300, hi: 4800 } },
  { t: 0.52, F: [285, 2230, 3000, 3500], amp: 0.82, f0: 135, n: { g: 0.002, lo: 300, hi: 4800 } },
  { t: 0.78, F: [720, 1150, 2480, 3300], amp: 0.9, f0: 124, n: { g: 0.002, lo: 300, hi: 4800 } },
  { t: 1.12, F: [730, 1130, 2500, 3300], amp: 0.92, f0: 118, n: { g: 0.002, lo: 300, hi: 4800 } },
  { t: 1.36, F: [310, 880, 2250, 3000], amp: 0.8, f0: 108, n: { g: 0.002, lo: 300, hi: 4800 } },
  { t: 1.72, F: [305, 870, 2240, 3000], amp: 0.7, f0: 98, n: { g: 0.002, lo: 300, hi: 4800 } },
  { t: 1.95, F: [400, 1200, 2400, 3200], amp: 0.08, f0: 0, n: { g: 0.006, lo: 300, hi: 4800 } },
  { t: 2.1, F: [500, 1500, 2500, 3300], amp: 0, f0: 0, n: { g: 0.004, lo: 300, hi: 4800 } }
];

export const WAV_KEYS: Key[] = [
  { t: 0, F: [500, 1500, 2500, 3300], amp: 0.01, f0: 0, n: { g: 0.002, lo: 200, hi: 3800 } },
  { t: 0.18, F: [280, 2250, 3000, 3500], amp: 0.7, f0: 128, n: { g: 0.002, lo: 200, hi: 3800 } },
  { t: 0.5, F: [285, 2230, 3000, 3500], amp: 0.72, f0: 134, n: { g: 0.002, lo: 200, hi: 3800 } },
  { t: 0.72, F: [720, 1150, 2480, 3300], amp: 0.9, f0: 122, n: { g: 0.002, lo: 200, hi: 3800 } },
  { t: 1.05, F: [730, 1130, 2500, 3300], amp: 0.85, f0: 112, n: { g: 0.002, lo: 200, hi: 3800 } },
  { t: 1.22, F: [400, 1400, 2500, 3300], amp: 0.05, f0: 0, n: { g: 0.003, lo: 200, hi: 3800 } },
  { t: 1.3, F: [500, 1500, 2500, 3300], amp: 0.01, f0: 0, n: { g: 0.002, lo: 200, hi: 3800 } }
];
