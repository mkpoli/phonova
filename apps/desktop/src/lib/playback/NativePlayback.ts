import { invoke } from '@tauri-apps/api/core';
import type { Playback } from './Playback';

/** The status every playback command returns: seconds and play state. */
type PlaybackStatus = { position: number; playing: boolean; duration: number };

/**
 * The desktop's native playback transport: each control is one Tauri command
 * into the `phx-playback` cpal engine, whose cursor comes from an atomic sample
 * counter in the audio callback. The synchronous {@link position} and
 * {@link playing} the cursor reads are a cache refreshed by a short poll of
 * `playback_status`, since the engine lives across the IPC boundary.
 */
export class NativePlayback implements Playback {
  #position = 0;
  #playing = false;
  #duration = 0;
  #poll: ReturnType<typeof setInterval> | null = null;

  /** Reports whether the host opened a native output device. */
  static async available(): Promise<boolean> {
    try {
      return await invoke<boolean>('playback_available');
    } catch {
      return false;
    }
  }

  #apply(status: PlaybackStatus) {
    this.#position = status.position;
    this.#playing = status.playing;
    this.#duration = status.duration;
  }

  #startPolling() {
    if (this.#poll) return;
    this.#poll = setInterval(() => {
      void invoke<PlaybackStatus>('playback_status')
        .then((status) => this.#apply(status))
        .catch(() => {
          // The stream fell away; keep the last cursor rather than resetting.
        });
    }, 30);
  }

  async load(file: File): Promise<void> {
    const bytes = new Uint8Array(await file.arrayBuffer());
    this.#apply(await invoke<PlaybackStatus>('playback_load', bytes));
    this.#startPolling();
  }

  async toggle(cursorTime: number): Promise<boolean> {
    if (this.#playing) {
      this.pause();
      return false;
    }
    await this.play(cursorTime);
    return true;
  }

  async play(cursorTime: number): Promise<void> {
    this.#apply(await invoke<PlaybackStatus>('playback_play', { seconds: cursorTime }));
    this.#startPolling();
  }

  async playRange(t0: number, t1: number): Promise<void> {
    this.#apply(await invoke<PlaybackStatus>('playback_play_range', { t0, t1 }));
    this.#startPolling();
  }

  pause(): void {
    this.#playing = false;
    void invoke<PlaybackStatus>('playback_pause')
      .then((status) => this.#apply(status))
      .catch(() => {});
  }

  stop(): void {
    this.#playing = false;
    this.#position = 0;
    void invoke<PlaybackStatus>('playback_stop')
      .then((status) => this.#apply(status))
      .catch(() => {});
  }

  seek(time: number): void {
    this.#position = time;
    void invoke<PlaybackStatus>('playback_seek', { seconds: time })
      .then((status) => this.#apply(status))
      .catch(() => {});
  }

  get position(): number {
    return this.#position;
  }

  get playing(): boolean {
    return this.#playing;
  }

  get duration(): number {
    return this.#duration;
  }

  close(): void {
    if (this.#poll) {
      clearInterval(this.#poll);
      this.#poll = null;
    }
    void invoke('playback_stop').catch(() => {});
  }
}
