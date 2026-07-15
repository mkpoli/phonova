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

/** Stable annotation-document handle. */
export type AnnotationId = bigint;
/** Stable tier handle within a document. */
export type TierId = bigint;
/** Stable boundary handle within a document. */
export type BoundaryId = bigint;
/** Stable interval handle within a tier. */
export type IntervalId = bigint;
/** Stable point handle within a tier. */
export type PointId = bigint;

export type TierKind = 'interval' | 'point';

/** One tier's identity and kind, in document order. */
export interface TierInfo {
  id: TierId;
  name: string;
  kind: TierKind;
}

/** A labeled interval bounded by two stable boundary ids. */
export interface IntervalData {
  id: IntervalId;
  startBoundary: BoundaryId;
  endBoundary: BoundaryId;
  xmin: number;
  xmax: number;
  label: string;
}

/** A labeled point at a time. */
export interface PointData {
  id: PointId;
  time: number;
  label: string;
}

/** A cross-document label search hit. */
export interface LabelHit {
  annotation: AnnotationId;
  tier: TierId;
  kind: TierKind;
  target: bigint;
  start: number;
  end: number;
}

/** What a command, undo, or redo changed, for incremental UI patching. */
export interface AppliedChange {
  kind: string;
  annotation?: AnnotationId;
  audio?: AudioId;
  tier?: TierId;
  boundary?: BoundaryId;
}

/** The journaled annotation surface: every mutator routes through the engine. */
export interface AnnotationClientLike {
  createAnnotation(audioId: AudioId, xmin: number, xmax: number): Promise<AnnotationId>;
  addIntervalTier(annotationId: AnnotationId, name: string): Promise<TierId>;
  addPointTier(annotationId: AnnotationId, name: string): Promise<TierId>;
  removeTier(annotationId: AnnotationId, tierId: TierId): Promise<AppliedChange>;
  insertBoundary(annotationId: AnnotationId, tierId: TierId, at: number): Promise<BoundaryId>;
  moveBoundary(
    annotationId: AnnotationId,
    boundaryId: BoundaryId,
    to: number,
    linked: boolean
  ): Promise<AppliedChange>;
  removeBoundary(annotationId: AnnotationId, boundaryId: BoundaryId): Promise<AppliedChange>;
  setIntervalLabel(
    annotationId: AnnotationId,
    tierId: TierId,
    intervalId: IntervalId,
    text: string
  ): Promise<AppliedChange>;
  setPointLabel(
    annotationId: AnnotationId,
    tierId: TierId,
    pointId: PointId,
    text: string
  ): Promise<AppliedChange>;
  undo(): Promise<AppliedChange | null>;
  redo(): Promise<AppliedChange | null>;
  undoDepth(): Promise<number>;
  redoDepth(): Promise<number>;
  stateHash(): Promise<bigint>;
  annotationTiers(annotationId: AnnotationId): Promise<TierInfo[]>;
  intervalsInRange(
    annotationId: AnnotationId,
    tierId: TierId,
    t0: number,
    t1: number
  ): Promise<IntervalData[]>;
  pointsInRange(
    annotationId: AnnotationId,
    tierId: TierId,
    t0: number,
    t1: number
  ): Promise<PointData[]>;
  searchLabels(pattern: string, regex: boolean): Promise<LabelHit[]>;
  importTextGrid(audioId: AudioId, bytes: Uint8Array): Promise<AnnotationId>;
  exportTextGrid(annotationId: AnnotationId): Promise<Uint8Array>;
}

export interface CoreClientLike extends AnnotationClientLike {
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
