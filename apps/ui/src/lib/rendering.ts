export const FRAME_FALLBACK_MS = 32;
const FRAME_WINDOW = 24;

export class FrameTimeMonitor {
  #samples: number[] = [];

  record(ms: number): boolean {
    if (!Number.isFinite(ms)) return false;
    this.#samples.push(ms);
    if (this.#samples.length > FRAME_WINDOW) this.#samples.shift();
    if (this.#samples.length < FRAME_WINDOW) return false;
    const average = this.#samples.reduce((sum, value) => sum + value, 0) / this.#samples.length;
    return average > FRAME_FALLBACK_MS;
  }
}

export function statsFromSamples(samples: number[]) {
  if (samples.length === 0) return { p50: 0, p95: 0, max: 0, samples: 0 };
  const sorted = [...samples].sort((a, b) => a - b);
  const at = (q: number) => sorted[Math.min(sorted.length - 1, Math.floor(q * (sorted.length - 1)))] ?? 0;
  return {
    p50: at(0.5),
    p95: at(0.95),
    max: sorted[sorted.length - 1] ?? 0,
    samples: samples.length
  };
}

export function makeProgram(gl: WebGL2RenderingContext, vertexSource: string, fragmentSource: string) {
  const vertex = compileShader(gl, gl.VERTEX_SHADER, vertexSource);
  const fragment = compileShader(gl, gl.FRAGMENT_SHADER, fragmentSource);
  const program = gl.createProgram();
  if (!program) throw new Error('WebGL program allocation failed');
  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    throw new Error(gl.getProgramInfoLog(program) || 'WebGL program link failed');
  }
  return program;
}

function compileShader(gl: WebGL2RenderingContext, type: number, source: string) {
  const shader = gl.createShader(type);
  if (!shader) throw new Error('WebGL shader allocation failed');
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    throw new Error(gl.getShaderInfoLog(shader) || 'WebGL shader compile failed');
  }
  return shader;
}

/**
 * The backing-store size a canvas's CSS box calls for, without touching the
 * canvas yet. Mutating `canvas.width`/`height` clears the backing store
 * immediately, so a caller that must await fresh pixels before redrawing
 * (a wasm slice or tile fetch) measures first with this, keeps showing the
 * old bitmap (the browser scales it to the new CSS box like an image) for
 * the duration of that fetch, and only calls {@link applyCanvasSize} once the
 * new pixels are ready to draw in the same tick — a resize never shows a
 * blank frame.
 */
export function measureCanvasTarget(canvas: HTMLCanvasElement) {
  const dpr = Math.max(1, window.devicePixelRatio || 1);
  const width = Math.max(1, Math.floor(canvas.clientWidth * dpr));
  const height = Math.max(1, Math.floor(canvas.clientHeight * dpr));
  return { width, height, dpr };
}

/** Resizes the backing store to `width`×`height` device pixels, a no-op if already that size. */
export function applyCanvasSize(canvas: HTMLCanvasElement, width: number, height: number) {
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
}

/**
 * Measures and immediately applies the backing-store size. Safe for a caller
 * that redraws synchronously right after (no `await` between this call and
 * the draw), since there's no window where the canvas sits cleared.
 */
export function resizeCanvas(canvas: HTMLCanvasElement) {
  const target = measureCanvasTarget(canvas);
  applyCanvasSize(canvas, target.width, target.height);
  return target;
}

/**
 * A viewport a canvas's pixels were last rasterized for. Panes keep this
 * alongside the live viewport so a pan or zoom can redraw the existing pixels
 * with a CSS transform immediately, before fresh imagery streams in.
 */
export interface DrawnViewport {
  t0: number;
  t1: number;
  /** Vertical anchor: frequency floor/ceiling (spectrogram) or amplitude scale (waveform). */
  vLo: number;
  vHi: number;
}

/**
 * CSS transform (origin `0 0`) that maps imagery rasterized for `base` onto the
 * `live` viewport, so waveform, spectrogram, and overlays move as one rigid
 * sheet. `vertical` selects how the vertical axis is interpreted: `'freq'` maps
 * a value range increasing upward, `'amp'` scales symmetrically about the
 * mid-line. Returns `'none'` when `base` matches `live` (the settled state).
 */
export function slippyTransform(
  base: DrawnViewport,
  live: DrawnViewport,
  vertical: 'freq' | 'amp'
): string {
  const timeSpan = live.t1 - live.t0;
  const sx = timeSpan !== 0 ? (base.t1 - base.t0) / timeSpan : 1;
  const tx = timeSpan !== 0 ? ((base.t0 - live.t0) / timeSpan) * 100 : 0;

  let sy = 1;
  let ty = 0;
  if (vertical === 'freq') {
    const vSpan = live.vHi - live.vLo;
    if (vSpan !== 0) {
      sy = (base.vHi - base.vLo) / vSpan;
      ty = (1 - (base.vHi - live.vLo) / vSpan) * 100;
    }
  } else {
    sy = base.vLo !== 0 ? live.vLo / base.vLo : 1;
    ty = 50 * (1 - sy);
  }

  if (sx === 1 && tx === 0 && sy === 1 && ty === 0) return 'none';
  // Percentages resolve against the element box, so the transform is
  // resolution-independent and needs no pixel dimensions.
  return `translate(${tx}%, ${ty}%) scale(${sx}, ${sy})`;
}

export function cssVar(name: string, fallback: string) {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

export function hexToRgb01(hex: string) {
  const normalized = hex.startsWith('#') ? hex.slice(1) : hex;
  const value = Number.parseInt(normalized.length === 3 ? normalized.replace(/(.)/g, '$1$1') : normalized, 16);
  return [((value >> 16) & 255) / 255, ((value >> 8) & 255) / 255, (value & 255) / 255] as const;
}
