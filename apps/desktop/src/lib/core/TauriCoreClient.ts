import { invoke } from '@tauri-apps/api/core';
import type {
  AnnotationId,
  AppliedChange,
  AudioId,
  AudioInfo,
  BoundaryId,
  CoreClientLike,
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
} from '@phonia/ui';

/** Widens a command's numeric id back to the `bigint` handle the UI passes. */
const big = (value: number | string): bigint => BigInt(value);
/** Narrows a `bigint` handle to the number a command argument carries. */
const num = (value: bigint): number => Number(value);

/** Raw numeric shapes the commands return, before id widening. */
type AppliedRaw = {
  kind: string;
  annotation?: number;
  audio?: number;
  tier?: number;
  boundary?: number;
};

function appliedFrom(raw: AppliedRaw | null): AppliedChange | null {
  if (!raw) return null;
  const out: AppliedChange = { kind: raw.kind };
  if (raw.annotation !== undefined) out.annotation = big(raw.annotation);
  if (raw.audio !== undefined) out.audio = big(raw.audio);
  if (raw.tier !== undefined) out.tier = big(raw.tier);
  if (raw.boundary !== undefined) out.boundary = big(raw.boundary);
  return out;
}

/**
 * The desktop transport of the shared {@link CoreClient} interface: every method
 * is one Tauri command into the native `phx-engine` behind a `Mutex<Engine>`,
 * mirroring the web worker protocol so the shared UI runs unchanged. Ids cross
 * the boundary as `u64`; this client widens them to `bigint` on the way out and
 * narrows them on the way in. Bulk buffers (waveform, spectrogram) cross as raw
 * bytes; everything else is JSON.
 */
export class TauriCoreClient implements CoreClientLike {
  async importAudio(src: File | string): Promise<AudioInfo> {
    const file = typeof src === 'string' ? await fileFromUrl(src) : src;
    const bytes = new Uint8Array(await file.arrayBuffer());
    const info = await invoke<{
      id: number;
      duration: number;
      sampleRate: number;
      channels: number;
      name?: string;
      hash: string;
    }>('import_audio', bytes);
    return {
      id: big(info.id),
      duration: info.duration,
      sampleRate: info.sampleRate,
      channels: info.channels,
      name: info.name ?? file.name,
      hash: info.hash
    };
  }

  /**
   * Opens an audio file already stored under the projects root, letting the
   * native side choose the eager or streamed path by its length.
   *
   * `rel` is a `/`-separated path beneath the root. A file over the engine's
   * eager frame threshold opens streamed over a `Send + Sync` file reader, so
   * the whole decoded signal never enters native memory; a shorter one decodes
   * whole, exactly as {@link importAudio} does.
   */
  async openAudioStreaming(rel: string, name: string): Promise<AudioInfo> {
    const info = await invoke<{
      id: number;
      duration: number;
      sampleRate: number;
      channels: number;
      name?: string;
      hash: string;
    }>('open_audio_streaming', { rel });
    return {
      id: big(info.id),
      duration: info.duration,
      sampleRate: info.sampleRate,
      channels: info.channels,
      name: info.name ?? name,
      hash: info.hash
    };
  }

  async waveformSlice(id: AudioId, t0: number, t1: number, px: number): Promise<MinMaxPyramidSlice> {
    const buffer = await invoke<ArrayBuffer>('waveform_slice', { id: num(id), t0, t1, px });
    return { t0, t1, px, data: new Float32Array(buffer) };
  }

  async samplesInRange(id: AudioId, t0: number, t1: number): Promise<Float32Array> {
    const buffer = await invoke<ArrayBuffer>('samples_in_range', { id: num(id), t0, t1 });
    return new Float32Array(buffer);
  }

  async spectrogramTile(id: AudioId, req: SpectrogramTileRequest): Promise<ImageBitmap> {
    const buffer = await invoke<ArrayBuffer>('spectrogram_tile', { id: num(id), req });
    const clamped = new Uint8ClampedArray(buffer);
    const image = new ImageData(clamped, req.widthPx, req.heightPx);
    return createImageBitmap(image);
  }

  async pitchTrack(id: AudioId, floorHz: number, ceilingHz: number): Promise<PitchTrackData> {
    return toPitch(await invoke('pitch_track', { id: num(id), floorHz, ceilingHz }));
  }

  async pitchTrackSpan(
    id: AudioId,
    floorHz: number,
    ceilingHz: number,
    t0: number,
    t1: number
  ): Promise<PitchTrackData> {
    return toPitch(await invoke('pitch_track_span', { id: num(id), floorHz, ceilingHz, t0, t1 }));
  }

  async formantTrack(
    id: AudioId,
    ceilingHz: number,
    maxFormants: number,
    smoothed: boolean
  ): Promise<FormantTrackData> {
    const raw = await invoke<{ points: number[]; maxHz: number }>('formant_track', {
      id: num(id),
      ceilingHz,
      maxFormants,
      smoothed
    });
    return { points: new Float64Array(raw.points), maxHz: raw.maxHz };
  }

  async intensityTrack(id: AudioId, floorHz: number): Promise<IntensityTrackData> {
    const raw = await invoke<{ times: number[]; db: number[] }>('intensity_track', {
      id: num(id),
      floorHz
    });
    return { times: new Float64Array(raw.times), db: new Float64Array(raw.db) };
  }

  bandEnergy(id: AudioId, t0: number, t1: number, f0: number, f1: number): Promise<number> {
    return invoke('band_energy', { id: num(id), t0, t1, f0, f1 });
  }

  async bandFilteredSpan(
    id: AudioId,
    t0: number,
    t1: number,
    fLow: number,
    fHigh: number
  ): Promise<Float32Array> {
    const buffer = await invoke<ArrayBuffer>('band_filtered_span', {
      id: num(id),
      t0,
      t1,
      fLow,
      fHigh
    });
    return new Float32Array(buffer);
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
    return invoke('selection_readout', {
      id: num(id),
      t0,
      t1,
      f0,
      f1,
      pitchFloorHz,
      pitchCeilingHz,
      intensityFloorHz
    });
  }

  async formantSpanMeans(
    id: AudioId,
    ceilingHz: number,
    maxFormants: number,
    smoothed: boolean,
    t0: number,
    t1: number
  ): Promise<Float64Array> {
    const means = await invoke<number[]>('formant_span_means', {
      id: num(id),
      ceilingHz,
      maxFormants,
      smoothed,
      t0,
      t1
    });
    return new Float64Array(means);
  }

  voiceReport(
    id: AudioId,
    t0: number,
    t1: number,
    pitchFloorHz: number,
    pitchCeilingHz: number
  ): Promise<VoiceReportData> {
    return invoke('voice_report', { id: num(id), t0, t1, pitchFloorHz, pitchCeilingHz });
  }

  async createAnnotation(audioId: AudioId, xmin: number, xmax: number): Promise<AnnotationId> {
    return big(await invoke<number>('create_annotation', { audioId: num(audioId), xmin, xmax }));
  }

  async addIntervalTier(annotationId: AnnotationId, name: string): Promise<TierId> {
    return big(await invoke<number>('add_interval_tier', { annotationId: num(annotationId), name }));
  }

  async addPointTier(annotationId: AnnotationId, name: string): Promise<TierId> {
    return big(await invoke<number>('add_point_tier', { annotationId: num(annotationId), name }));
  }

  async removeTier(annotationId: AnnotationId, tierId: TierId): Promise<AppliedChange> {
    return appliedFrom(
      await invoke('remove_tier', { annotationId: num(annotationId), tierId: num(tierId) })
    ) as AppliedChange;
  }

  async insertBoundary(annotationId: AnnotationId, tierId: TierId, at: number): Promise<BoundaryId> {
    return big(
      await invoke<number>('insert_boundary', {
        annotationId: num(annotationId),
        tierId: num(tierId),
        at
      })
    );
  }

  async moveBoundary(
    annotationId: AnnotationId,
    boundaryId: BoundaryId,
    to: number,
    linked: boolean
  ): Promise<AppliedChange> {
    return appliedFrom(
      await invoke('move_boundary', {
        annotationId: num(annotationId),
        boundaryId: num(boundaryId),
        to,
        linked
      })
    ) as AppliedChange;
  }

  async removeBoundary(
    annotationId: AnnotationId,
    boundaryId: BoundaryId
  ): Promise<AppliedChange> {
    return appliedFrom(
      await invoke('remove_boundary', {
        annotationId: num(annotationId),
        boundaryId: num(boundaryId)
      })
    ) as AppliedChange;
  }

  async setIntervalLabel(
    annotationId: AnnotationId,
    tierId: TierId,
    intervalId: IntervalId,
    text: string
  ): Promise<AppliedChange> {
    return appliedFrom(
      await invoke('set_interval_label', {
        annotationId: num(annotationId),
        tierId: num(tierId),
        intervalId: num(intervalId),
        text
      })
    ) as AppliedChange;
  }

  async setPointLabel(
    annotationId: AnnotationId,
    tierId: TierId,
    pointId: PointId,
    text: string
  ): Promise<AppliedChange> {
    return appliedFrom(
      await invoke('set_point_label', {
        annotationId: num(annotationId),
        tierId: num(tierId),
        pointId: num(pointId),
        text
      })
    ) as AppliedChange;
  }

  async renameAudio(audioId: AudioId, name: string): Promise<AppliedChange> {
    return appliedFrom(
      await invoke<AppliedRaw>('rename_audio', { audioId: num(audioId), name })
    ) as AppliedChange;
  }

  async detachAudio(audioId: AudioId): Promise<AppliedChange> {
    return appliedFrom(
      await invoke<AppliedRaw>('detach_audio', { audioId: num(audioId) })
    ) as AppliedChange;
  }

  async undo(): Promise<AppliedChange | null> {
    return appliedFrom(await invoke<AppliedRaw | null>('undo'));
  }

  async redo(): Promise<AppliedChange | null> {
    return appliedFrom(await invoke<AppliedRaw | null>('redo'));
  }

  undoDepth(): Promise<number> {
    return invoke('undo_depth');
  }

  redoDepth(): Promise<number> {
    return invoke('redo_depth');
  }

  async stateHash(): Promise<bigint> {
    return big(await invoke<string>('state_hash'));
  }

  async listAnnotations(audioId: AudioId): Promise<AnnotationId[]> {
    const ids = await invoke<number[]>('list_annotations', { audioId: num(audioId) });
    return ids.map(big);
  }

  async annotationTiers(annotationId: AnnotationId): Promise<TierInfo[]> {
    const tiers = await invoke<{ id: number; name: string; kind: 'interval' | 'point' }[]>(
      'annotation_tiers',
      { annotationId: num(annotationId) }
    );
    return tiers.map((tier) => ({ id: big(tier.id), name: tier.name, kind: tier.kind }));
  }

  async intervalsInRange(
    annotationId: AnnotationId,
    tierId: TierId,
    t0: number,
    t1: number
  ): Promise<IntervalData[]> {
    const rows = await invoke<
      {
        id: number;
        startBoundary: number;
        endBoundary: number;
        xmin: number;
        xmax: number;
        label: string;
      }[]
    >('intervals_in_range', { annotationId: num(annotationId), tierId: num(tierId), t0, t1 });
    return rows.map((row) => ({
      id: big(row.id),
      startBoundary: big(row.startBoundary),
      endBoundary: big(row.endBoundary),
      xmin: row.xmin,
      xmax: row.xmax,
      label: row.label
    }));
  }

  async pointsInRange(
    annotationId: AnnotationId,
    tierId: TierId,
    t0: number,
    t1: number
  ): Promise<PointData[]> {
    const rows = await invoke<{ id: number; time: number; label: string }[]>('points_in_range', {
      annotationId: num(annotationId),
      tierId: num(tierId),
      t0,
      t1
    });
    return rows.map((row) => ({ id: big(row.id), time: row.time, label: row.label }));
  }

  async searchLabels(pattern: string, regex: boolean): Promise<LabelHit[]> {
    const rows = await invoke<
      {
        annotation: number;
        tier: number;
        kind: 'interval' | 'point';
        target: number;
        start: number;
        end: number;
      }[]
    >('search_labels', { pattern, regex });
    return rows.map((row) => ({
      annotation: big(row.annotation),
      tier: big(row.tier),
      kind: row.kind,
      target: big(row.target),
      start: row.start,
      end: row.end
    }));
  }

  async importTextGrid(audioId: AudioId, bytes: Uint8Array): Promise<AnnotationId> {
    return big(
      await invoke<number>('import_text_grid', { audioId: num(audioId), bytes: Array.from(bytes) })
    );
  }

  async exportTextGrid(annotationId: AnnotationId): Promise<Uint8Array> {
    const bytes = await invoke<number[]>('export_text_grid', {
      annotationId: num(annotationId)
    });
    return Uint8Array.from(bytes);
  }

  annotationJson(annotationId: AnnotationId): Promise<string> {
    return invoke('annotation_json', { annotationId: num(annotationId) });
  }

  async attachAnnotationJson(audioId: AudioId, json: string): Promise<AnnotationId> {
    return big(await invoke<number>('attach_annotation_json', { audioId: num(audioId), json }));
  }

  async saveProjectContainer(spec: SaveProjectSpec): Promise<Uint8Array> {
    const bytes = await invoke<number[]>('save_project_container', {
      specJson: JSON.stringify(spec)
    });
    return Uint8Array.from(bytes);
  }

  loadProjectContainer(bytes: Uint8Array): Promise<LoadedProjectContainer> {
    return invoke('load_project_container', { bytes: Array.from(bytes) });
  }

  async renameProjectContainer(bytes: Uint8Array, name: string): Promise<Uint8Array> {
    const out = await invoke<number[]>('rename_project_container', {
      bytes: Array.from(bytes),
      name
    });
    return Uint8Array.from(out);
  }

  buildFigure(spec: FigureSpec): Promise<string> {
    return invoke('build_figure', { specJson: JSON.stringify(spec) });
  }

  renderFigureSvg(figureJson: string): Promise<string> {
    return invoke('render_figure_svg', { figureJson });
  }

  async exportFigure(figureJson: string, format: FigureExportFormat): Promise<FigureExportResult> {
    const bundle = await invoke<{
      mainName: string;
      mainBytes: number[];
      mime: string;
      isText: boolean;
      sidecars: { name: string; bytes: number[] }[];
    }>('export_figure', { figureJson, format });
    return {
      mainName: bundle.mainName,
      mainBytes: Uint8Array.from(bundle.mainBytes),
      mime: bundle.mime,
      isText: bundle.isText,
      sidecars: bundle.sidecars.map((s) => ({ name: s.name, bytes: Uint8Array.from(s.bytes) }))
    };
  }

  // The web client tears down its worker here; the native engine lives in the
  // Rust process, so this only exists to satisfy the shell's cleanup call.
  destroy() {}
}

function toPitch(raw: { times: number[]; f0: number[]; maxHz: number }): PitchTrackData {
  return { times: new Float64Array(raw.times), f0: new Float64Array(raw.f0), maxHz: raw.maxHz };
}

async function fileFromUrl(url: string): Promise<File> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Audio request failed: ${response.status}`);
  const blob = await response.blob();
  const name = url.split('/').pop() || 'audio.wav';
  return new File([blob], name, { type: blob.type || 'audio/wav' });
}
