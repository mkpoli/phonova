export class WebAudioPlayback {
  #context: AudioContext | null = null;
  #buffer: AudioBuffer | null = null;
  #source: AudioBufferSourceNode | null = null;
  #startedAt = 0;
  #offset = 0;
  #playing = false;
  // Loop state: `#loop` is the user's toggle; `#loopStart`/`#loopEnd` are the
  // bounds of whatever range is currently sounding (the whole file for
  // `play()`, a span for `playRange()`), kept live so a mid-playback toggle
  // wraps the segment that is actually playing, not a stale one.
  #loop = false;
  #loopStart = 0;
  #loopEnd = 0;

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
    this.#loopStart = 0;
    this.#loopEnd = this.#buffer.duration;
    source.loop = this.#loop;
    source.loopStart = this.#loopStart;
    source.loopEnd = this.#loopEnd;
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
    this.#loopStart = start;
    this.#loopEnd = end > start ? end : duration;
    source.loop = this.#loop;
    source.loopStart = this.#loopStart;
    source.loopEnd = this.#loopEnd;
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

  /** Toggles whether the currently sounding (or next) range repeats. Live —
   *  it takes effect on the source that is playing right now, not just the
   *  next one started. */
  setLoop(enabled: boolean) {
    this.#loop = enabled;
    if (this.#source) this.#source.loop = enabled;
  }

  get loop() {
    return this.#loop;
  }

  /**
   * Plays a raw mono sample buffer once, at `sampleRate`, from the start, and
   * resolves when it ends. Used for a band-filtered box selection, whose audio
   * is rendered by the engine rather than sliced from the loaded file. This is a
   * preview: it does not become the transport's playing state and leaves the
   * file cursor where it was.
   */
  async playBuffer(samples: Float32Array, sampleRate: number): Promise<void> {
    if (samples.length === 0) return;
    const context = this.#audioContext();
    await context.resume();
    this.stopSource();
    this.#playing = false;
    const buffer = context.createBuffer(1, samples.length, sampleRate);
    buffer.getChannelData(0).set(samples);
    const source = context.createBufferSource();
    source.buffer = buffer;
    source.connect(context.destination);
    this.#source = source;
    await new Promise<void>((resolve) => {
      source.onended = () => {
        if (this.#source === source) this.#source = null;
        resolve();
      };
      source.start(0);
    });
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
    const elapsed = this.#context.currentTime - this.#startedAt;
    if (!this.#loop || !this.#source?.loop) return Math.min(duration, this.#offset + elapsed);
    // Looping: the first lap runs from the start offset to loopEnd (which may
    // be shorter than a full lap), every lap after that spans the full
    // loopStart..loopEnd range, wrapping the elapsed clock to match.
    const span = Math.max(0.0001, this.#loopEnd - this.#loopStart);
    const firstLap = this.#loopEnd - this.#offset;
    if (elapsed < firstLap) return this.#offset + elapsed;
    return this.#loopStart + ((elapsed - firstLap) % span);
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
