import type {
  AudioId,
  AudioInfo,
  FormantTrackData,
  IntensityTrackData,
  MinMaxPyramidSlice,
  PitchTrackData,
  SpectrogramTileRequest
} from '@phonix/ui';

export type {
  AudioId,
  AudioInfo,
  FormantTrackData,
  IntensityTrackData,
  MinMaxPyramidSlice,
  PitchTrackData,
  SpectrogramTileRequest
};

export interface Applied {
  revision: number;
}

export interface CoreClient {
  importAudio(src: File | string): Promise<AudioInfo>;
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
  apply(cmd: unknown): Promise<Applied>;
  undo(): Promise<void>;
}
