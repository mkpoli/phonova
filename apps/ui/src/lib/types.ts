export type AudioId = bigint;

export interface AudioInfo {
  id: AudioId;
  duration: number;
  sampleRate: number;
  channels: number;
  name?: string;
  /** BLAKE3 content hash of the source bytes, 64 lowercase hex characters. */
  hash?: string;
}

export interface MinMaxPyramidSlice {
  t0: number;
  t1: number;
  px: number;
  data: Float32Array;
}

export type WasmColormapName = 'Viridis' | 'Magma' | 'Inferno' | 'Plasma' | 'Cividis' | 'Grayscale';
export type WasmThemeName = 'Light' | 'Dark';

/** WAV output sample format: 16/24/32-bit PCM or lossless 32-bit float. */
export type WavBitDepth = 'Pcm16' | 'Pcm24' | 'Pcm32' | 'Float32';

/** How a project file carries its recordings when exported. */
export type ProjectExportMode = 'bundle' | 'references';

/** What the editor's audio-export dialog resolves to when the user downloads. */
export interface AudioExportOptions {
  scope: 'whole' | 'selection';
  bits: WavBitDepth;
  /** Band-limit the selection to its box frequencies (box selection only). */
  filtered: boolean;
}

/** A resolved audio-export request the editor hands the shell to encode. */
export interface AudioExportRequest {
  scope: 'whole' | 'selection';
  t0: number;
  t1: number;
  f0: number;
  f1: number;
  bits: WavBitDepth;
  filtered: boolean;
}

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
  /** Every live document attached to `audioId`, ascending by id (most recently attached last). */
  listAnnotations(audioId: AudioId): Promise<AnnotationId[]>;
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
  /** Renames a stored recording. Journaled; undo restores the prior name. */
  renameAudio(audioId: AudioId, name: string): Promise<AppliedChange>;
  /**
   * Detaches a recording from the session, cascading to its annotation
   * documents. Journaled; undo restores the recording and those documents
   * together, keeping the same {@link AudioId}.
   */
  detachAudio(audioId: AudioId): Promise<AppliedChange>;
}

/** A node in the library tree: a recording leaf or a named group of nodes. */
export type LibraryNode = { Media: number } | { Group: LibraryGroup };

/** A named group of library nodes, nesting to any depth. */
export interface LibraryGroup {
  id: number;
  name: string;
  children: LibraryNode[];
}

/** One recording in a {@link SaveProjectSpec}, with session ids as plain numbers. */
export interface SaveProjectMediaSpec {
  mediaId: number;
  relativePath: string;
  hash: string;
  duration: number;
  sampleRate: number;
  channels: number;
  /** Session annotation id whose document is stored, or `null` when unannotated. */
  annotation: number | null;
  description: string;
  authors: string[];
  tags: string[];
}

/** The argument to {@link CoreClientLike.saveProjectContainer}. */
export interface SaveProjectSpec {
  name: string;
  savedAt: number;
  view: unknown;
  description: string;
  authors: string[];
  tags: string[];
  media: SaveProjectMediaSpec[];
  groups: LibraryNode[];
}

/** One recording parsed from a project container. */
export interface LoadedProjectMedia {
  mediaId: number;
  relativePath: string;
  hash: string;
  duration: number;
  sampleRate: number;
  channels: number;
  /** Serialized annotation document to re-attach after import, or `null`. */
  annotationJson: string | null;
  description: string;
  authors: string[];
  tags: string[];
}

/** A project container parsed back into the metadata a session restores from. */
export interface LoadedProjectContainer {
  name: string;
  savedAt: number;
  view: unknown;
  description: string;
  authors: string[];
  tags: string[];
  media: LoadedProjectMedia[];
  groups: LibraryNode[];
}

/** One recording inside an open project, bound to its live session ids. */
export interface RecordingEntry {
  /** Stable id within the project, independent of the session's audio ids. */
  mediaId: number;
  /** Display name (the file stem). */
  name: string;
  /** Basename stored under the project's `audio/` directory. */
  fileName: string;
  /** Path relative to the project file, using `/`. */
  relativePath: string;
  /** BLAKE3 content hash, 64 lowercase hex characters. */
  hash: string;
  duration: number;
  sampleRate: number;
  channels: number;
  /** Live audio id once the recording is decoded into the session. */
  audioId: AudioId | null;
  /** Live annotation id once a document is attached. */
  annotationId: AnnotationId | null;
  /** Whether the recording carries an annotation document. */
  hasAnnotation: boolean;
  /** Free-form description of this recording. Empty when unset. */
  description: string;
  /** Contributors credited for this recording, in listing order. */
  authors: string[];
  /** Free-form tags applied to this recording, in listing order. */
  tags: string[];
}

/** A project as listed on the home grid, without its media decoded. */
export interface ProjectSummary {
  id: string;
  name: string;
  savedAt: number;
  count: number;
  /** A newer autosave sidecar holds unsaved work from an interrupted session. */
  hasRecovery: boolean;
}

/** How a selection was drawn, and therefore what it bounds. */
export type SelectionMode = 'time' | 'box';

/**
 * A selection in signal coordinates, so it stays anchored to the signal across
 * zoom and pan. A `time` selection (waveform drag) spans the visible frequency
 * range; a `box` selection (spectrogram drag) bounds frequency too.
 */
export interface Selection {
  t0: number;
  t1: number;
  f0: number;
  f1: number;
  mode: SelectionMode;
}

/**
 * The measurement readout for a selection. Every number is an engine query over
 * the box, so the readout bar equals what a script reading the same API returns
 * (the batch-equals-GUI invariant). Absent measures are `null`.
 */
export interface SelectionReadout {
  t0: number;
  t1: number;
  f0: number;
  f1: number;
  duration: number;
  f0MeanHz: number | null;
  f0MinHz: number | null;
  f0MaxHz: number | null;
  bandEnergyDb: number;
  intensityMeanDb: number | null;
  hnrMeanDb: number | null;
}

/** The aggregate voice report over a span, as the report card renders it. */
export interface VoiceReportData {
  t0: number;
  t1: number;
  pitch: { meanHz: number | null; medianHz: number | null; minHz: number | null; maxHz: number | null };
  jitter: {
    local: number | null;
    localAbsolute: number | null;
    rap: number | null;
    ppq5: number | null;
    ddp: number | null;
  };
  shimmer: {
    local: number | null;
    localDb: number | null;
    apq3: number | null;
    apq5: number | null;
    apq11: number | null;
    dda: number | null;
  };
  meanHnrDb: number | null;
  cppDb: number;
  cppsDb: number | null;
  voiceBreaks: { thresholdSeconds: number; totalSeconds: number; count: number };
  moments: {
    centreOfGravityHz: number | null;
    standardDeviationHz: number | null;
    skewness: number | null;
    kurtosis: number | null;
  };
  pulseCount: number;
  params: {
    pitchFloorHz: number;
    pitchCeilingHz: number;
    harmonicityFloorHz: number;
    periodsPerWindow: number;
    cppFrameLengthSeconds: number;
    cppMinF0Hz: number;
    cppMaxF0Hz: number;
  };
}

export interface CoreClientLike extends AnnotationClientLike {
  bandEnergy(id: AudioId, t0: number, t1: number, f0: number, f1: number): Promise<number>;
  /**
   * Renders `[t0, t1]` of `id` band-filtered to `[fLow, fHigh]` as a mono
   * `Float32Array` at the source sample rate, for audible playback of a box
   * selection.
   */
  bandFilteredSpan(
    id: AudioId,
    t0: number,
    t1: number,
    fLow: number,
    fHigh: number
  ): Promise<Float32Array>;
  selectionReadout(
    id: AudioId,
    t0: number,
    t1: number,
    f0: number,
    f1: number,
    pitchFloorHz: number,
    pitchCeilingHz: number,
    intensityFloorHz: number
  ): Promise<SelectionReadout>;
  formantSpanMeans(
    id: AudioId,
    ceilingHz: number,
    maxFormants: number,
    smoothed: boolean,
    t0: number,
    t1: number
  ): Promise<Float64Array>;
  voiceReport(
    id: AudioId,
    t0: number,
    t1: number,
    pitchFloorHz: number,
    pitchCeilingHz: number
  ): Promise<VoiceReportData>;
  annotationJson(annotationId: AnnotationId): Promise<string>;
  attachAnnotationJson(audioId: AudioId, json: string): Promise<AnnotationId>;
  saveProjectContainer(spec: SaveProjectSpec): Promise<Uint8Array>;
  loadProjectContainer(bytes: Uint8Array): Promise<LoadedProjectContainer>;
  renameProjectContainer(bytes: Uint8Array, name: string): Promise<Uint8Array>;
  waveformSlice(id: AudioId, t0: number, t1: number, px: number): Promise<MinMaxPyramidSlice>;
  /** Exact unfiltered mono samples of `[t0, t1]` at the source rate, for the
   *  waveform pane's sample-accurate polyline at high zoom. */
  samplesInRange(id: AudioId, t0: number, t1: number): Promise<Float32Array>;
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
  buildFigure(spec: FigureSpec): Promise<string>;
  renderFigureSvg(figureJson: string): Promise<string>;
  exportFigure(figureJson: string, format: FigureExportFormat): Promise<FigureExportResult>;
}

export type FigureLengthUnit = 'cm' | 'in' | 'pt';
export type FigureThemeName = 'light' | 'dark';
export type FigureColormapName =
  | 'viridis'
  | 'magma'
  | 'inferno'
  | 'plasma'
  | 'cividis'
  | 'grayscale';
export type FigurePitchUnitName = 'hertz' | 'semitones';
export type FigureExportFormat =
  | 'svg'
  | 'png'
  | 'pdf'
  | 'vega'
  | 'tikz'
  | 'typst'
  | 'python'
  | 'r'
  | 'julia'
  | 'graphml';

/** Per-layer inclusion for a figure, mirroring the editor overlays. */
export interface FigureLayerToggles {
  waveform: boolean;
  spectrogram: boolean;
  pitch: boolean;
  formant: boolean;
  intensity: boolean;
  tiers: boolean;
}

/**
 * A figure build request. Field names match the engine's serde wire format, so
 * this object serializes straight to the `buildFigure` argument. Ids are plain
 * numbers here (the engine reads them as integers), not the bigint handles the
 * annotation surface uses.
 */
export interface FigureSpec {
  audio: number;
  annotation: number | null;
  t0: number;
  t1: number;
  f0: number;
  f1: number;
  layers: FigureLayerToggles;
  width: number;
  height: number;
  unit: FigureLengthUnit;
  theme: FigureThemeName;
  colormap: FigureColormapName;
  dynamic_range_db: number;
  max_db: number | null;
  spectrogram_width_px: number;
  spectrogram_height_px: number;
  window_length: number;
  pitch_floor_hz: number;
  pitch_ceiling_hz: number;
  pitch_unit: FigurePitchUnitName;
  formant_ceiling_hz: number;
  formant_max: number;
  formant_smoothed: boolean;
  intensity_floor_hz: number;
}

/** A named file emitted alongside an export's main document. */
export interface FigureSidecar {
  name: string;
  bytes: Uint8Array;
}

/** The result of a figure export: main document plus any sidecar files. */
export interface FigureExportResult {
  mainName: string;
  mainBytes: Uint8Array;
  mime: string;
  isText: boolean;
  sidecars: FigureSidecar[];
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
