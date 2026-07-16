import type {
  AnnotationId,
  AppliedChange,
  AudioId,
  AudioInfo,
  BoundaryId,
  CoreClient,
  FigureExportFormat,
  FigureExportResult,
  FigureSpec,
  FormantTrackData,
  IntensityTrackData,
  IntervalData,
  IntervalId,
  LabelHit,
  LoadedProjectContainer,
  MinMaxPyramidSlice,
  PitchTrackData,
  PointData,
  PointId,
  SaveProjectSpec,
  SelectionReadout,
  SpectrogramTileRequest,
  TierId,
  TierInfo,
  VoiceReportData
} from './types';

/** A finished recording: its live audio id, metadata, and WAV bytes to persist. */
export interface FinishedRecordingResult {
  audioId: AudioId;
  duration: number;
  sampleRate: number;
  channels: number;
  hash: string;
  wav: Uint8Array;
}

type Pending = {
  resolve: (value: unknown) => void;
  reject: (reason?: unknown) => void;
};

type WorkerResponse =
  | { id: number; ok: true; result: unknown }
  | { id: number; ok: false; error: string };

export class WasmCoreClient implements CoreClient {
  #worker: Worker;
  #nextId = 1;
  #pending = new Map<number, Pending>();

  constructor() {
    this.#worker = new Worker(new URL('./wasm-worker.ts', import.meta.url), { type: 'module' });
    this.#worker.onmessage = (event: MessageEvent<WorkerResponse>) => {
      const message = event.data;
      const pending = this.#pending.get(message.id);
      if (!pending) return;
      this.#pending.delete(message.id);
      if (message.ok) pending.resolve(message.result);
      else pending.reject(new Error(message.error));
    };
  }

  async importAudio(src: File | string): Promise<AudioInfo> {
    const file = typeof src === 'string' ? await fileFromUrl(src) : src;
    const bytes = await file.arrayBuffer();
    return this.#call<AudioInfo>({ method: 'importAudio', bytes, name: file.name }, [bytes]);
  }

  beginRecording(sampleRate: number, channels: number): Promise<bigint> {
    return this.#call({ method: 'beginRecording', sampleRate, channels });
  }

  appendSamples(recordingId: bigint, samples: Float32Array): Promise<void> {
    const buffer = samples.buffer.slice(
      samples.byteOffset,
      samples.byteOffset + samples.byteLength
    );
    return this.#call({ method: 'appendSamples', recordingId, samples: buffer }, [buffer]);
  }

  finishRecording(recordingId: bigint, name: string): Promise<FinishedRecordingResult> {
    return this.#call({ method: 'finishRecording', recordingId, name });
  }

  abortRecording(recordingId: bigint): Promise<void> {
    return this.#call({ method: 'abortRecording', recordingId });
  }

  waveformSlice(id: AudioId, t0: number, t1: number, px: number): Promise<MinMaxPyramidSlice> {
    return this.#call({ method: 'waveformSlice', audioId: id, t0, t1, px });
  }

  async spectrogramTile(id: AudioId, req: SpectrogramTileRequest): Promise<ImageBitmap> {
    const result = await this.#call<{ width: number; height: number; rgba: Uint8Array }>({
      method: 'spectrogramTile',
      audioId: id,
      req
    });
    const clamped = new Uint8ClampedArray(result.rgba.byteLength);
    clamped.set(result.rgba);
    const image = new ImageData(clamped, result.width, result.height);
    return createImageBitmap(image);
  }

  /**
   * Times an isolated STFT+colorize against a recolor-only pass over the same
   * viewport, and reports the raw-dB block count before and after the recolor.
   * A recolor must not grow the count. Used by the perf probe; not part of the
   * rendering path.
   */
  spectrogramProbe(
    id: AudioId,
    req: SpectrogramTileRequest
  ): Promise<{
    stftMs: number;
    recolorMs: number;
    blocksAfterStft: number;
    blocksAfterRecolor: number;
  }> {
    return this.#call({ method: 'spectrogramProbe', audioId: id, req });
  }

  pitchTrack(id: AudioId, floorHz: number, ceilingHz: number): Promise<PitchTrackData> {
    return this.#call({ method: 'pitchTrack', audioId: id, floorHz, ceilingHz });
  }

  pitchTrackSpan(
    id: AudioId,
    floorHz: number,
    ceilingHz: number,
    t0: number,
    t1: number
  ): Promise<PitchTrackData> {
    return this.#call({ method: 'pitchTrackSpan', audioId: id, floorHz, ceilingHz, t0, t1 });
  }

  formantTrack(
    id: AudioId,
    ceilingHz: number,
    maxFormants: number,
    smoothed: boolean
  ): Promise<FormantTrackData> {
    return this.#call({ method: 'formantTrack', audioId: id, ceilingHz, maxFormants, smoothed });
  }

  intensityTrack(id: AudioId, floorHz: number): Promise<IntensityTrackData> {
    return this.#call({ method: 'intensityTrack', audioId: id, floorHz });
  }

  bandEnergy(id: AudioId, t0: number, t1: number, f0: number, f1: number): Promise<number> {
    return this.#call({ method: 'bandEnergy', audioId: id, t0, t1, f0, f1 });
  }

  selectionReadout(
    id: AudioId,
    t0: number,
    t1: number,
    f0: number,
    f1: number,
    pitchFloorHz: number,
    pitchCeilingHz: number,
    intensityFloorHz: number
  ): Promise<SelectionReadout> {
    return this.#call({
      method: 'selectionReadout',
      audioId: id,
      t0,
      t1,
      f0,
      f1,
      pitchFloorHz,
      pitchCeilingHz,
      intensityFloorHz
    });
  }

  formantSpanMeans(
    id: AudioId,
    ceilingHz: number,
    maxFormants: number,
    smoothed: boolean,
    t0: number,
    t1: number
  ): Promise<Float64Array> {
    return this.#call({
      method: 'formantSpanMeans',
      audioId: id,
      ceilingHz,
      maxFormants,
      smoothed,
      t0,
      t1
    });
  }

  voiceReport(
    id: AudioId,
    t0: number,
    t1: number,
    pitchFloorHz: number,
    pitchCeilingHz: number
  ): Promise<VoiceReportData> {
    return this.#call({ method: 'voiceReport', audioId: id, t0, t1, pitchFloorHz, pitchCeilingHz });
  }

  createAnnotation(audioId: AudioId, xmin: number, xmax: number): Promise<AnnotationId> {
    return this.#call({ method: 'createAnnotation', audioId, xmin, xmax });
  }

  addIntervalTier(annotationId: AnnotationId, name: string): Promise<TierId> {
    return this.#call({ method: 'addIntervalTier', annotationId, name });
  }

  addPointTier(annotationId: AnnotationId, name: string): Promise<TierId> {
    return this.#call({ method: 'addPointTier', annotationId, name });
  }

  removeTier(annotationId: AnnotationId, tierId: TierId): Promise<AppliedChange> {
    return this.#call({ method: 'removeTier', annotationId, tierId });
  }

  insertBoundary(annotationId: AnnotationId, tierId: TierId, at: number): Promise<BoundaryId> {
    return this.#call({ method: 'insertBoundary', annotationId, tierId, at });
  }

  moveBoundary(
    annotationId: AnnotationId,
    boundaryId: BoundaryId,
    to: number,
    linked: boolean
  ): Promise<AppliedChange> {
    return this.#call({ method: 'moveBoundary', annotationId, boundaryId, to, linked });
  }

  removeBoundary(annotationId: AnnotationId, boundaryId: BoundaryId): Promise<AppliedChange> {
    return this.#call({ method: 'removeBoundary', annotationId, boundaryId });
  }

  setIntervalLabel(
    annotationId: AnnotationId,
    tierId: TierId,
    intervalId: IntervalId,
    text: string
  ): Promise<AppliedChange> {
    return this.#call({ method: 'setIntervalLabel', annotationId, tierId, intervalId, text });
  }

  setPointLabel(
    annotationId: AnnotationId,
    tierId: TierId,
    pointId: PointId,
    text: string
  ): Promise<AppliedChange> {
    return this.#call({ method: 'setPointLabel', annotationId, tierId, pointId, text });
  }

  undo(): Promise<AppliedChange | null> {
    return this.#call({ method: 'undo' });
  }

  redo(): Promise<AppliedChange | null> {
    return this.#call({ method: 'redo' });
  }

  undoDepth(): Promise<number> {
    return this.#call({ method: 'undoDepth' });
  }

  redoDepth(): Promise<number> {
    return this.#call({ method: 'redoDepth' });
  }

  stateHash(): Promise<bigint> {
    return this.#call({ method: 'stateHash' });
  }

  listAnnotations(audioId: AudioId): Promise<AnnotationId[]> {
    return this.#call({ method: 'listAnnotations', audioId });
  }

  annotationTiers(annotationId: AnnotationId): Promise<TierInfo[]> {
    return this.#call({ method: 'annotationTiers', annotationId });
  }

  intervalsInRange(
    annotationId: AnnotationId,
    tierId: TierId,
    t0: number,
    t1: number
  ): Promise<IntervalData[]> {
    return this.#call({ method: 'intervalsInRange', annotationId, tierId, t0, t1 });
  }

  pointsInRange(
    annotationId: AnnotationId,
    tierId: TierId,
    t0: number,
    t1: number
  ): Promise<PointData[]> {
    return this.#call({ method: 'pointsInRange', annotationId, tierId, t0, t1 });
  }

  searchLabels(pattern: string, regex: boolean): Promise<LabelHit[]> {
    return this.#call({ method: 'searchLabels', pattern, regex });
  }

  importTextGrid(audioId: AudioId, bytes: Uint8Array): Promise<AnnotationId> {
    const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    return this.#call({ method: 'importTextGrid', audioId, bytes: buffer }, [buffer]);
  }

  exportTextGrid(annotationId: AnnotationId): Promise<Uint8Array> {
    return this.#call({ method: 'exportTextGrid', annotationId });
  }

  annotationJson(annotationId: AnnotationId): Promise<string> {
    return this.#call({ method: 'annotationJson', annotationId });
  }

  attachAnnotationJson(audioId: AudioId, json: string): Promise<AnnotationId> {
    return this.#call({ method: 'attachAnnotationJson', audioId, json });
  }

  saveProjectContainer(spec: SaveProjectSpec): Promise<Uint8Array> {
    return this.#call({ method: 'saveProjectContainer', spec });
  }

  async loadProjectContainer(bytes: Uint8Array): Promise<LoadedProjectContainer> {
    const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    const json = await this.#call<string>({ method: 'loadProjectContainer', bytes: buffer }, [
      buffer
    ]);
    return JSON.parse(json) as LoadedProjectContainer;
  }

  renameProjectContainer(bytes: Uint8Array, name: string): Promise<Uint8Array> {
    const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    return this.#call({ method: 'renameProjectContainer', bytes: buffer, name }, [buffer]);
  }

  buildFigure(spec: FigureSpec): Promise<string> {
    return this.#call({ method: 'buildFigure', spec });
  }

  renderFigureSvg(figureJson: string): Promise<string> {
    return this.#call({ method: 'renderFigureSvg', figureJson });
  }

  exportFigure(figureJson: string, format: FigureExportFormat): Promise<FigureExportResult> {
    return this.#call({ method: 'exportFigure', figureJson, format });
  }

  destroy() {
    this.#worker.terminate();
    this.#pending.clear();
  }

  #call<T>(payload: Record<string, unknown>, transfer: Transferable[] = []): Promise<T> {
    const id = this.#nextId++;
    const promise = new Promise<T>((resolve, reject) => {
      this.#pending.set(id, { resolve: resolve as (value: unknown) => void, reject });
    });
    this.#worker.postMessage({ id, ...payload }, transfer);
    return promise;
  }
}

async function fileFromUrl(url: string) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Audio request failed: ${response.status}`);
  const blob = await response.blob();
  const name = url.split('/').pop() || 'audio.wav';
  return new File([blob], name, { type: blob.type || 'audio/wav' });
}
