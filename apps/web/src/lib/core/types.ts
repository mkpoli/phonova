import type {
  AudioId,
  AudioInfo,
  MinMaxPyramidSlice,
  SpectrogramTileRequest
} from '@phonix/ui';

export type { AudioId, AudioInfo, MinMaxPyramidSlice, SpectrogramTileRequest };

export interface Applied {
  revision: number;
}

export interface PitchTrack {
  times: Float64Array;
  values: Float64Array;
}

export interface CoreClient {
  importAudio(src: File | string): Promise<AudioInfo>;
  waveformSlice(id: AudioId, t0: number, t1: number, px: number): Promise<MinMaxPyramidSlice>;
  spectrogramTile(id: AudioId, req: SpectrogramTileRequest): Promise<ImageBitmap>;
  pitchTrack(id: AudioId, params: Record<string, unknown>): Promise<PitchTrack>;
  apply(cmd: unknown): Promise<Applied>;
  undo(): Promise<void>;
}
