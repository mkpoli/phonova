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

/** A pitch contour: parallel arrays, `f0` holding `NaN` on unvoiced frames. */
export interface PitchTrackData {
  times: Float64Array;
  f0: Float64Array;
  maxHz: number;
}

/** Formant candidates as flat `[time, frequency, bandwidth]` triples. */
export interface FormantTrackData {
  points: Float64Array;
  maxHz: number;
}

/** An intensity contour: parallel arrays of frame times and dB SPL levels. */
export interface IntensityTrackData {
  times: Float64Array;
  db: Float64Array;
}

export interface CoreClientLike {
  waveformSlice(id: AudioId, t0: number, t1: number, px: number): Promise<MinMaxPyramidSlice>;
  spectrogramTile(id: AudioId, req: SpectrogramTileRequest): Promise<ImageBitmap>;
  pitchTrack(id: AudioId, floorHz: number, ceilingHz: number): Promise<PitchTrackData>;
  pitchTrackSpan(
    id: AudioId,
    floorHz: number,
    ceilingHz: number,
    t0: number,
    t1: number
  ): Promise<PitchTrackData>;
  formantTrack(
    id: AudioId,
    ceilingHz: number,
    maxFormants: number,
    smoothed: boolean
  ): Promise<FormantTrackData>;
  intensityTrack(id: AudioId, floorHz: number): Promise<IntensityTrackData>;
}

/** Per-track visibility and analysis parameters edited in the inspector. */
export interface OverlayParams {
  pitch: { show: boolean; floorHz: number; ceilingHz: number };
  formant: { show: boolean; ceilingHz: number; maxFormants: number; smoothed: boolean };
  intensity: { show: boolean; floorHz: number };
}

/** Highest tracked value per track, for the inspector's clipping badges. */
export interface OverlayStats {
  pitchMaxHz: number;
  formantMaxHz: number;
}

export function defaultOverlayParams(): OverlayParams {
  return {
    pitch: { show: true, floorHz: 75, ceilingHz: 600 },
    formant: { show: true, ceilingHz: 5500, maxFormants: 5, smoothed: false },
    intensity: { show: true, floorHz: 100 }
  };
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
