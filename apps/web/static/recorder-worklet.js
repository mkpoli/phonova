// Capture processor for microphone recording.
//
// Runs on the audio render thread. It collects the raw input blocks (128-frame
// quanta), batches them to roughly 100 ms before posting a planar Float32 chunk
// to the main thread, and posts a lighter level message (RMS, peak, clip flag)
// about thirty times a second for the meter. No filtering or gain is applied —
// phonetics needs the signal as the device delivers it.

class RecorderProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    const opts = options.processorOptions || {};
    const rate = sampleRate; // global in the AudioWorkletGlobalScope
    this.channels = Math.max(1, opts.channels || 1);
    this.batchFrames = Math.max(128, Math.round((rate * (opts.batchMs || 100)) / 1000));
    this.levelFrames = Math.max(128, Math.round((rate * (opts.levelMs || 33)) / 1000));

    // Per-channel lists of copied blocks awaiting the next chunk post.
    this.pending = Array.from({ length: this.channels }, () => []);
    this.pendingFrames = 0;

    // Level accumulators reset on each meter post.
    this.levelSumSquares = 0;
    this.levelSampleCount = 0;
    this.levelPeak = 0;
    this.levelFrameCount = 0;

    this.stopped = false;
    this.port.onmessage = (event) => {
      if (event.data && event.data.command === 'stop') {
        this.flushChunk(true);
        this.stopped = true;
      }
    };
  }

  flushChunk(final) {
    if (this.pendingFrames === 0) {
      if (final) this.port.postMessage({ type: 'chunk', frames: 0, samples: new Float32Array(0), final: true });
      return;
    }
    const frames = this.pendingFrames;
    const out = new Float32Array(this.channels * frames);
    for (let c = 0; c < this.channels; c += 1) {
      let offset = c * frames;
      for (const block of this.pending[c]) {
        out.set(block, offset);
        offset += block.length;
      }
      this.pending[c] = [];
    }
    this.pendingFrames = 0;
    this.port.postMessage({ type: 'chunk', frames, samples: out, final: Boolean(final) }, [out.buffer]);
  }

  postLevel() {
    const rms = this.levelSampleCount > 0 ? Math.sqrt(this.levelSumSquares / this.levelSampleCount) : 0;
    this.port.postMessage({
      type: 'level',
      rms,
      peak: this.levelPeak,
      clipped: this.levelPeak >= 0.999
    });
    this.levelSumSquares = 0;
    this.levelSampleCount = 0;
    this.levelPeak = 0;
    this.levelFrameCount = 0;
  }

  process(inputs) {
    if (this.stopped) return false;
    const input = inputs[0];
    if (!input || input.length === 0 || !input[0]) {
      // No live input this block (device warming up); keep the node alive.
      return true;
    }
    const frames = input[0].length;

    for (let c = 0; c < this.channels; c += 1) {
      const source = input[c] || input[0];
      // The render buffer is reused after process() returns, so copy it.
      this.pending[c].push(Float32Array.from(source));
    }
    this.pendingFrames += frames;

    // Level statistics over the first channel drive the meter.
    const meterSource = input[0];
    for (let i = 0; i < frames; i += 1) {
      const value = meterSource[i];
      this.levelSumSquares += value * value;
      const magnitude = value < 0 ? -value : value;
      if (magnitude > this.levelPeak) this.levelPeak = magnitude;
    }
    this.levelSampleCount += frames;
    this.levelFrameCount += frames;

    if (this.levelFrameCount >= this.levelFrames) this.postLevel();
    if (this.pendingFrames >= this.batchFrames) this.flushChunk(false);

    return true;
  }
}

registerProcessor('recorder-processor', RecorderProcessor);
