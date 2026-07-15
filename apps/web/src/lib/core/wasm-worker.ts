import init, { WasmColormap, WasmEngine, WasmTheme } from '../wasm/pkg/phx_wasm.js';
import type { AudioId, SpectrogramTileRequest } from './types';

type RequestMessage =
  | { id: number; method: 'importAudio'; bytes: ArrayBuffer; name: string }
  | { id: number; method: 'waveformSlice'; audioId: AudioId; t0: number; t1: number; px: number }
  | { id: number; method: 'spectrogramTile'; audioId: AudioId; req: SpectrogramTileRequest }
  | { id: number; method: 'apply'; cmd: unknown }
  | { id: number; method: 'undo' }
  | { id: number; method: 'pitchTrack'; audioId: AudioId; params: Record<string, unknown> };

type ResponseMessage =
  | { id: number; ok: true; result: unknown; transfer?: never }
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

self.onmessage = async (event: MessageEvent<RequestMessage>) => {
  const message = event.data;
  try {
    const wasm = await engine();
    if (message.method === 'importAudio') {
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
    if (message.method === 'waveformSlice') {
      const data = wasm.waveformSlice(message.audioId, message.t0, message.t1, message.px);
      const copy = new Float32Array(data.length);
      copy.set(data);
      postMessage(
        { id: message.id, ok: true, result: { t0: message.t0, t1: message.t1, px: message.px, data: copy } },
        { transfer: [copy.buffer] }
      );
      return;
    }
    if (message.method === 'spectrogramTile') {
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
    if (message.method === 'apply') {
      postMessage({ id: message.id, ok: true, result: { revision: 0 } } satisfies ResponseMessage);
      return;
    }
    if (message.method === 'undo') {
      postMessage({ id: message.id, ok: true, result: undefined } satisfies ResponseMessage);
      return;
    }
    postMessage({ id: message.id, ok: true, result: { times: new Float64Array(), values: new Float64Array() } });
  } catch (error) {
    postMessage({ id: message.id, ok: false, error: error instanceof Error ? error.message : String(error) } satisfies ResponseMessage);
  }
};
