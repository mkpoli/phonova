import init, {
	WasmColormap,
	WasmEngine,
	WasmTheme,
	WasmTierRelation,
	exportFigure as wasmExportFigure,
	renderFigureSvg as wasmRenderFigureSvg
} from '../wasm/pkg/phx_wasm.js';
import type {
  AnnotationId,
  AudioId,
  BoundaryId,
  FigureExportFormat,
  FigureSpec,
  IntervalId,
  PointId,
  SpectrogramTileRequest,
  TierId,
  TierInfo
} from './types';

type RequestMessage =
  | { id: number; method: 'importAudio'; bytes: ArrayBuffer; name: string }
  | { id: number; method: 'waveformSlice'; audioId: AudioId; t0: number; t1: number; px: number }
  | { id: number; method: 'spectrogramTile'; audioId: AudioId; req: SpectrogramTileRequest }
  | { id: number; method: 'pitchTrack'; audioId: AudioId; floorHz: number; ceilingHz: number }
  | {
      id: number;
      method: 'pitchTrackSpan';
      audioId: AudioId;
      floorHz: number;
      ceilingHz: number;
      t0: number;
      t1: number;
    }
  | {
      id: number;
      method: 'formantTrack';
      audioId: AudioId;
      ceilingHz: number;
      maxFormants: number;
      smoothed: boolean;
    }
  | { id: number; method: 'intensityTrack'; audioId: AudioId; floorHz: number }
  | { id: number; method: 'createAnnotation'; audioId: AudioId; xmin: number; xmax: number }
  | { id: number; method: 'addIntervalTier'; annotationId: AnnotationId; name: string }
  | { id: number; method: 'addPointTier'; annotationId: AnnotationId; name: string }
  | { id: number; method: 'removeTier'; annotationId: AnnotationId; tierId: TierId }
  | { id: number; method: 'insertBoundary'; annotationId: AnnotationId; tierId: TierId; at: number }
  | {
      id: number;
      method: 'moveBoundary';
      annotationId: AnnotationId;
      boundaryId: BoundaryId;
      to: number;
      linked: boolean;
    }
  | { id: number; method: 'removeBoundary'; annotationId: AnnotationId; boundaryId: BoundaryId }
  | {
      id: number;
      method: 'setIntervalLabel';
      annotationId: AnnotationId;
      tierId: TierId;
      intervalId: IntervalId;
      text: string;
    }
  | {
      id: number;
      method: 'setPointLabel';
      annotationId: AnnotationId;
      tierId: TierId;
      pointId: PointId;
      text: string;
    }
  | { id: number; method: 'undo' }
  | { id: number; method: 'redo' }
  | { id: number; method: 'undoDepth' }
  | { id: number; method: 'redoDepth' }
  | { id: number; method: 'stateHash' }
  | { id: number; method: 'annotationTiers'; annotationId: AnnotationId }
  | {
      id: number;
      method: 'intervalsInRange';
      annotationId: AnnotationId;
      tierId: TierId;
      t0: number;
      t1: number;
    }
  | {
      id: number;
      method: 'pointsInRange';
      annotationId: AnnotationId;
      tierId: TierId;
      t0: number;
      t1: number;
    }
  | { id: number; method: 'searchLabels'; pattern: string; regex: boolean }
  | { id: number; method: 'importTextGrid'; audioId: AudioId; bytes: ArrayBuffer }
  | { id: number; method: 'exportTextGrid'; annotationId: AnnotationId }
  | { id: number; method: 'buildFigure'; spec: FigureSpec }
  | { id: number; method: 'renderFigureSvg'; figureJson: string }
  | { id: number; method: 'exportFigure'; figureJson: string; format: FigureExportFormat };

type ResponseMessage =
  | { id: number; ok: true; result: unknown }
  | { id: number; ok: false; error: string };

let enginePromise: Promise<WasmEngine> | null = null;

function engine() {
  enginePromise ??= init().then(() => new WasmEngine());
  return enginePromise;
}

type SyncAccessHandleLike = {
  truncate(size: number): void;
  write(buffer: BufferSource, options?: { at?: number }): number;
  flush(): void;
  close(): void;
};

async function storeInOpfs(audioId: AudioId, bytes: Uint8Array<ArrayBuffer>) {
  const root = await navigator.storage?.getDirectory?.();
  if (!root) return false;
  const file = await root.getFileHandle(`audio-${String(audioId)}.wav`, { create: true });
  const handleSource = file as FileSystemFileHandle & {
    createSyncAccessHandle?: () => Promise<SyncAccessHandleLike>;
  };
  if (handleSource.createSyncAccessHandle) {
    const handle = await handleSource.createSyncAccessHandle();
    handle.truncate(0);
    handle.write(bytes, { at: 0 });
    handle.flush();
    handle.close();
    return true;
  }
  const writable = await file.createWritable();
  await writable.write(bytes);
  await writable.close();
  return true;
}

function colormap(name: SpectrogramTileRequest['colormap']): WasmColormap {
  return WasmColormap[name];
}

function theme(name: SpectrogramTileRequest['theme']): WasmTheme {
  return WasmTheme[name];
}

type WasmAppliedLike = {
  kind: string;
  annotation?: bigint;
  audio?: bigint;
  tier?: bigint;
  boundary?: bigint;
};

function appliedToPlain(applied: WasmAppliedLike | undefined | null) {
  if (!applied) return null;
  return {
    kind: applied.kind,
    annotation: applied.annotation,
    audio: applied.audio,
    tier: applied.tier,
    boundary: applied.boundary
  };
}

function tiersToPlain(list: {
  ids: BigUint64Array;
  names: string[];
  kinds: Uint8Array;
}): TierInfo[] {
  const out: TierInfo[] = [];
  for (let i = 0; i < list.ids.length; i += 1) {
    out.push({ id: list.ids[i], name: list.names[i], kind: list.kinds[i] === 1 ? 'point' : 'interval' });
  }
  return out;
}

function intervalsToPlain(list: {
  ids: BigUint64Array;
  startBoundaries: BigUint64Array;
  endBoundaries: BigUint64Array;
  xmin: Float64Array;
  xmax: Float64Array;
  labels: string[];
}) {
  const out = [];
  for (let i = 0; i < list.ids.length; i += 1) {
    out.push({
      id: list.ids[i],
      startBoundary: list.startBoundaries[i],
      endBoundary: list.endBoundaries[i],
      xmin: list.xmin[i],
      xmax: list.xmax[i],
      label: list.labels[i]
    });
  }
  return out;
}

function pointsToPlain(list: { ids: BigUint64Array; times: Float64Array; labels: string[] }) {
  const out = [];
  for (let i = 0; i < list.ids.length; i += 1) {
    out.push({ id: list.ids[i], time: list.times[i], label: list.labels[i] });
  }
  return out;
}

function hitsToPlain(list: {
  annotations: BigUint64Array;
  tiers: BigUint64Array;
  kinds: Uint8Array;
  targets: BigUint64Array;
  starts: Uint32Array;
  ends: Uint32Array;
}) {
  const out = [];
  for (let i = 0; i < list.annotations.length; i += 1) {
    out.push({
      annotation: list.annotations[i],
      tier: list.tiers[i],
      kind: list.kinds[i] === 1 ? ('point' as const) : ('interval' as const),
      target: list.targets[i],
      start: list.starts[i],
      end: list.ends[i]
    });
  }
  return out;
}

self.onmessage = async (event: MessageEvent<RequestMessage>) => {
  const message = event.data;
  try {
    const wasm = await engine();
    switch (message.method) {
      case 'importAudio': {
        const bytes = new Uint8Array(message.bytes);
        const importedId = wasm.importWavBytes(bytes);
        await storeInOpfs(importedId, bytes);
        const info = wasm.audioInfo(importedId);
        const result = {
          id: importedId,
          duration: info.duration,
          sampleRate: info.sampleRate,
          channels: info.channels,
          name: message.name || info.name || undefined
        };
        postMessage({ id: message.id, ok: true, result } satisfies ResponseMessage);
        return;
      }
      case 'waveformSlice': {
        const data = wasm.waveformSlice(message.audioId, message.t0, message.t1, message.px);
        const copy = new Float32Array(data.length);
        copy.set(data);
        postMessage(
          {
            id: message.id,
            ok: true,
            result: { t0: message.t0, t1: message.t1, px: message.px, data: copy }
          },
          { transfer: [copy.buffer] }
        );
        return;
      }
      case 'spectrogramTile': {
        const req = message.req;
        const data = wasm.spectrogramTileRgba(
          message.audioId,
          req.t0,
          req.t1,
          req.f0,
          req.f1,
          req.widthPx,
          req.heightPx,
          req.windowLength,
          req.maxFrequency,
          req.timeStep,
          req.frequencyStep,
          req.dynamicRangeDb,
          req.maxDb,
          colormap(req.colormap),
          theme(req.theme)
        );
        const copy = new Uint8Array(data.length);
        copy.set(data);
        postMessage(
          { id: message.id, ok: true, result: { width: req.widthPx, height: req.heightPx, rgba: copy } },
          { transfer: [copy.buffer] }
        );
        return;
      }
      case 'pitchTrack': {
        const track = wasm.pitchTrack(message.audioId, message.floorHz, message.ceilingHz);
        const times = new Float64Array(track.times);
        const f0 = new Float64Array(track.f0);
        postMessage(
          { id: message.id, ok: true, result: { times, f0, maxHz: track.maxHz } },
          { transfer: [times.buffer, f0.buffer] }
        );
        return;
      }
      case 'pitchTrackSpan': {
        const track = wasm.pitchTrackSpan(
          message.audioId,
          message.floorHz,
          message.ceilingHz,
          message.t0,
          message.t1
        );
        const times = new Float64Array(track.times);
        const f0 = new Float64Array(track.f0);
        postMessage(
          { id: message.id, ok: true, result: { times, f0, maxHz: track.maxHz } },
          { transfer: [times.buffer, f0.buffer] }
        );
        return;
      }
      case 'formantTrack': {
        const track = wasm.formantTrack(
          message.audioId,
          message.ceilingHz,
          message.maxFormants,
          message.smoothed
        );
        const points = new Float64Array(track.points);
        postMessage(
          { id: message.id, ok: true, result: { points, maxHz: track.maxHz } },
          { transfer: [points.buffer] }
        );
        return;
      }
      case 'intensityTrack': {
        const track = wasm.intensityTrack(message.audioId, message.floorHz);
        const times = new Float64Array(track.times);
        const db = new Float64Array(track.db);
        postMessage(
          { id: message.id, ok: true, result: { times, db } },
          { transfer: [times.buffer, db.buffer] }
        );
        return;
      }
      case 'createAnnotation': {
        const result = wasm.createAnnotation(message.audioId, message.xmin, message.xmax);
        postMessage({ id: message.id, ok: true, result } satisfies ResponseMessage);
        return;
      }
      case 'addIntervalTier': {
        const result = wasm.addIntervalTier(
          message.annotationId,
          message.name,
          WasmTierRelation.Independent,
          null
        );
        postMessage({ id: message.id, ok: true, result } satisfies ResponseMessage);
        return;
      }
      case 'addPointTier': {
        const result = wasm.addPointTier(message.annotationId, message.name);
        postMessage({ id: message.id, ok: true, result } satisfies ResponseMessage);
        return;
      }
      case 'removeTier': {
        const result = appliedToPlain(wasm.removeTier(message.annotationId, message.tierId));
        postMessage({ id: message.id, ok: true, result } satisfies ResponseMessage);
        return;
      }
      case 'insertBoundary': {
        const result = wasm.insertBoundary(message.annotationId, message.tierId, message.at);
        postMessage({ id: message.id, ok: true, result } satisfies ResponseMessage);
        return;
      }
      case 'moveBoundary': {
        const result = appliedToPlain(
          wasm.moveBoundary(message.annotationId, message.boundaryId, message.to, message.linked)
        );
        postMessage({ id: message.id, ok: true, result } satisfies ResponseMessage);
        return;
      }
      case 'removeBoundary': {
        const result = appliedToPlain(wasm.removeBoundary(message.annotationId, message.boundaryId));
        postMessage({ id: message.id, ok: true, result } satisfies ResponseMessage);
        return;
      }
      case 'setIntervalLabel': {
        const result = appliedToPlain(
          wasm.setIntervalLabel(message.annotationId, message.tierId, message.intervalId, message.text)
        );
        postMessage({ id: message.id, ok: true, result } satisfies ResponseMessage);
        return;
      }
      case 'setPointLabel': {
        const result = appliedToPlain(
          wasm.setPointLabel(message.annotationId, message.tierId, message.pointId, message.text)
        );
        postMessage({ id: message.id, ok: true, result } satisfies ResponseMessage);
        return;
      }
      case 'undo': {
        postMessage({ id: message.id, ok: true, result: appliedToPlain(wasm.undo()) } satisfies ResponseMessage);
        return;
      }
      case 'redo': {
        postMessage({ id: message.id, ok: true, result: appliedToPlain(wasm.redo()) } satisfies ResponseMessage);
        return;
      }
      case 'undoDepth': {
        postMessage({ id: message.id, ok: true, result: wasm.undoDepth() } satisfies ResponseMessage);
        return;
      }
      case 'redoDepth': {
        postMessage({ id: message.id, ok: true, result: wasm.redoDepth() } satisfies ResponseMessage);
        return;
      }
      case 'stateHash': {
        postMessage({ id: message.id, ok: true, result: wasm.stateHash() } satisfies ResponseMessage);
        return;
      }
      case 'annotationTiers': {
        const result = tiersToPlain(wasm.annotationTiers(message.annotationId));
        postMessage({ id: message.id, ok: true, result } satisfies ResponseMessage);
        return;
      }
      case 'intervalsInRange': {
        const result = intervalsToPlain(
          wasm.intervalsInRange(message.annotationId, message.tierId, message.t0, message.t1)
        );
        postMessage({ id: message.id, ok: true, result } satisfies ResponseMessage);
        return;
      }
      case 'pointsInRange': {
        const result = pointsToPlain(
          wasm.pointsInRange(message.annotationId, message.tierId, message.t0, message.t1)
        );
        postMessage({ id: message.id, ok: true, result } satisfies ResponseMessage);
        return;
      }
      case 'searchLabels': {
        const result = hitsToPlain(wasm.searchLabels(message.pattern, message.regex));
        postMessage({ id: message.id, ok: true, result } satisfies ResponseMessage);
        return;
      }
      case 'importTextGrid': {
        const bytes = new Uint8Array(message.bytes);
        const result = wasm.importTextGrid(message.audioId, bytes);
        postMessage({ id: message.id, ok: true, result } satisfies ResponseMessage);
        return;
      }
      case 'exportTextGrid': {
        const bytes = wasm.exportTextGrid(message.annotationId);
        const copy = new Uint8Array(bytes.length);
        copy.set(bytes);
        postMessage(
          { id: message.id, ok: true, result: copy },
          { transfer: [copy.buffer] }
        );
        return;
      }
      case 'buildFigure': {
        const result = wasm.buildFigure(JSON.stringify(message.spec));
        postMessage({ id: message.id, ok: true, result } satisfies ResponseMessage);
        return;
      }
      case 'renderFigureSvg': {
        const result = wasmRenderFigureSvg(message.figureJson);
        postMessage({ id: message.id, ok: true, result } satisfies ResponseMessage);
        return;
      }
      case 'exportFigure': {
        const bundle = wasmExportFigure(message.figureJson, message.format);
        const mainSource = bundle.mainBytes;
        const mainBytes = new Uint8Array(mainSource.length);
        mainBytes.set(mainSource);
        const sidecarNames = bundle.sidecarNames;
        const sidecars = sidecarNames.map((name, index) => {
          const source = bundle.sidecarBytes(index);
          const bytes = new Uint8Array(source.length);
          bytes.set(source);
          return { name, bytes };
        });
        const result = {
          mainName: bundle.mainName,
          mainBytes,
          mime: bundle.mime,
          isText: bundle.isText,
          sidecars
        };
        const transfer = [mainBytes.buffer, ...sidecars.map((s) => s.bytes.buffer)];
        postMessage({ id: message.id, ok: true, result }, { transfer });
        return;
      }
      default: {
        const unexpected: never = message;
        const unknownId = (unexpected as { id: number }).id;
        postMessage({ id: unknownId, ok: false, error: 'unknown method' } satisfies ResponseMessage);
      }
    }
  } catch (error) {
    postMessage({
      id: message.id,
      ok: false,
      error: error instanceof Error ? error.message : String(error)
    } satisfies ResponseMessage);
  }
};
