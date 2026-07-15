import type {
  Applied,
  AudioId,
  AudioInfo,
  CoreClient,
  MinMaxPyramidSlice,
  PitchTrack,
  SpectrogramTileRequest
} from './types';

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

  pitchTrack(id: AudioId, params: Record<string, unknown>): Promise<PitchTrack> {
    return this.#call({ method: 'pitchTrack', audioId: id, params });
  }

  apply(cmd: unknown): Promise<Applied> {
    return this.#call({ method: 'apply', cmd });
  }

  undo(): Promise<void> {
    return this.#call({ method: 'undo' });
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
