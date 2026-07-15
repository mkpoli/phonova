export type AudioId = bigint;

export interface AudioInfo {
  id: AudioId;
  duration: number;
  sampleRate: number;
  channels: number;
  name?: string;
}

export interface MinMaxPyramidSlice {
  t0: number;
  t1: number;
  px: number;
  data: Float32Array;
}

export type WasmColormapName = 'Viridis' | 'Magma' | 'Grayscale';
export type WasmThemeName = 'Light' | 'Dark';

export interface SpectrogramTileRequest {
  t0: number;
  t1: number;
  f0: number;
  f1: number;
  widthPx: number;
  heightPx: number;
  windowLength: number;
  maxFrequency: number;
  timeStep: number;
  frequencyStep: number;
  dynamicRangeDb: number;
  maxDb?: number;
  colormap: WasmColormapName;
  theme: WasmThemeName;
}

export interface CoreClientLike {
  waveformSlice(id: AudioId, t0: number, t1: number, px: number): Promise<MinMaxPyramidSlice>;
  spectrogramTile(id: AudioId, req: SpectrogramTileRequest): Promise<ImageBitmap>;
}

export interface ViewportState {
  t0: number;
  t1: number;
  ampScale: number;
  f0: number;
  f1: number;
}

export interface FrameStats {
  p50: number;
  p95: number;
  max: number;
  samples: number;
}

export function defaultViewport(duration = 1): ViewportState {
  const end = Math.max(duration, 0.001);
  return {
    t0: 0,
    t1: end,
    ampScale: 1,
    f0: 0,
    f1: 5000
  };
}

export function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds)) return '0.000';
  const clamped = Math.max(0, seconds);
  const minutes = Math.floor(clamped / 60);
  const rest = clamped - minutes * 60;
  return minutes > 0 ? `${minutes}:${rest.toFixed(3).padStart(6, '0')}` : rest.toFixed(3);
}

export function clampViewport(viewport: ViewportState, duration: number): ViewportState {
  const minSpan = 0.005;
  const maxDuration = Math.max(duration, minSpan);
  const span = Math.min(Math.max(viewport.t1 - viewport.t0, minSpan), maxDuration);
  let t0 = viewport.t0;
  let t1 = t0 + span;
  if (t0 < 0) {
    t0 = 0;
    t1 = span;
  }
  if (t1 > maxDuration) {
    t1 = maxDuration;
    t0 = Math.max(0, t1 - span);
  }
  return {
    ...viewport,
    t0,
    t1,
    ampScale: Math.min(Math.max(viewport.ampScale, 0.25), 8),
    f0: Math.max(0, viewport.f0),
    f1: Math.min(Math.max(viewport.f1, 100), 20000)
  };
}
