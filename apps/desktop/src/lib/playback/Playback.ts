/**
 * The playback control surface the editor shell drives, satisfied by both the
 * native cpal engine ({@link NativePlayback}) and the WebAudio fallback
 * ({@link WebAudioPlayback}). `position` and `playing` are synchronous reads the
 * animation-frame cursor polls; the native client keeps them fresh from the
 * engine's atomic sample counter behind the scenes.
 */
export interface Playback {
  /** Loads a WAV file, rewound and stopped. */
  load(file: File): Promise<void>;
  /** Toggles play/pause from `cursorTime`, returning the new playing state. */
  toggle(cursorTime: number): Promise<boolean>;
  /** Plays from `cursorTime` to the end. */
  play(cursorTime: number): Promise<void>;
  /** Plays the span `[t0, t1]`, stopping at its end. */
  playRange(t0: number, t1: number): Promise<void>;
  /** Stops, holding the cursor in place. */
  pause(): void;
  /** Stops and rewinds to the start. */
  stop(): void;
  /** Moves the cursor without changing play state. */
  seek(time: number): void;
  /** The current position in seconds. */
  readonly position: number;
  /** Whether playback is advancing. */
  readonly playing: boolean;
  /** Releases resources. */
  close(): void;
}
