export class WebAudioPlayback {
  #context: AudioContext | null = null;
  #buffer: AudioBuffer | null = null;
  #source: AudioBufferSourceNode | null = null;
  #startedAt = 0;
  #offset = 0;
  #playing = false;

  async load(file: File) {
    const context = this.#audioContext();
    const bytes = await file.arrayBuffer();
    this.stop();
    this.#buffer = await context.decodeAudioData(bytes.slice(0));
    this.#offset = 0;
  }

  async toggle(cursorTime: number) {
    if (this.#playing) {
      this.pause();
      return false;
    }
    await this.play(cursorTime);
    return true;
  }

  async play(cursorTime: number) {
    if (!this.#buffer) return;
    const context = this.#audioContext();
    await context.resume();
    this.stopSource();
    const source = context.createBufferSource();
    source.buffer = this.#buffer;
    source.connect(context.destination);
    const offset = Math.min(Math.max(0, cursorTime), Math.max(0, this.#buffer.duration - 0.001));
    source.start(0, offset);
    this.#offset = offset;
    this.#startedAt = context.currentTime;
    this.#source = source;
    this.#playing = true;
    source.onended = () => {
      if (this.#source === source) {
        this.#playing = false;
        this.#offset = Math.min(this.position, this.#buffer?.duration ?? this.#offset);
        this.#source = null;
      }
    };
  }

  async playRange(t0: number, t1: number) {
    if (!this.#buffer) return;
    const context = this.#audioContext();
    await context.resume();
    this.stopSource();
    const duration = this.#buffer.duration;
    const start = Math.min(Math.max(0, t0), Math.max(0, duration - 0.001));
    const end = Math.min(Math.max(start, t1), duration);
    const source = context.createBufferSource();
    source.buffer = this.#buffer;
    source.connect(context.destination);
    // A non-positive span falls back to playing from the start point.
    if (end - start > 0) source.start(0, start, end - start);
    else source.start(0, start);
    this.#offset = start;
    this.#startedAt = context.currentTime;
    this.#source = source;
    this.#playing = true;
    source.onended = () => {
      if (this.#source === source) {
        this.#playing = false;
        this.#offset = Math.min(this.position, this.#buffer?.duration ?? this.#offset);
        this.#source = null;
      }
    };
  }

  pause() {
    this.#offset = this.position;
    this.stopSource();
    this.#playing = false;
  }

  stop() {
    this.#offset = 0;
    this.stopSource();
    this.#playing = false;
  }

  seek(time: number) {
    const wasPlaying = this.#playing;
    this.#offset = Math.max(0, Math.min(time, this.#buffer?.duration ?? time));
    if (wasPlaying) void this.play(this.#offset);
  }

  get position() {
    if (!this.#playing || !this.#context) return this.#offset;
    const duration = this.#buffer?.duration ?? Number.POSITIVE_INFINITY;
    return Math.min(duration, this.#offset + (this.#context.currentTime - this.#startedAt));
  }

  get playing() {
    return this.#playing;
  }

  close() {
    this.stopSource();
    void this.#context?.close();
    this.#context = null;
  }

  #audioContext() {
    this.#context ??= new AudioContext();
    return this.#context;
  }

  private stopSource() {
    if (!this.#source) return;
    try {
      this.#source.stop();
    } catch {
      // Already stopped sources can throw in some browser engines.
    }
    this.#source.disconnect();
    this.#source = null;
  }
}
