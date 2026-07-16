/**
 * Microphone capture over getUserMedia and an AudioWorklet.
 *
 * The worklet on the render thread batches raw 128-frame quanta into ~100 ms
 * planar chunks and posts a level reading about thirty times a second. This
 * class opens the device, wires the graph, and forwards each chunk to a
 * consumer; it holds no engine reference, so the caller decides where samples
 * go. Capture is single-channel: phonetics works from one microphone signal,
 * and forcing mono keeps the take, the meter, and the persisted WAV consistent
 * regardless of how many channels the device reports.
 *
 * The capture constraints turn off echo cancellation, noise suppression, and
 * automatic gain: those processors reshape the waveform, and an acoustic
 * analysis needs the signal the microphone actually delivered.
 */

/** A meter reading from the render thread. */
export interface RecorderLevel {
  /** Root-mean-square level over the interval, in [0, 1]. */
  rms: number;
  /** Peak absolute sample over the interval, in [0, 1]. */
  peak: number;
  /** Whether the peak reached digital full scale (a clip). */
  clipped: boolean;
}

/** An input device the picker can offer. */
export interface RecorderDevice {
  deviceId: string;
  label: string;
}

/** One planar sample chunk plus its frame count. */
export interface RecorderChunk {
  samples: Float32Array;
  frames: number;
}

interface StartOptions {
  /** Device to open, or omitted for the system default. */
  deviceId?: string;
  /** Receives each planar sample chunk as it arrives. */
  onChunk: (chunk: RecorderChunk) => void;
  /** Receives each meter reading, about thirty times a second. */
  onLevel: (level: RecorderLevel) => void;
}

/** Whether this browser exposes the capture APIs the recorder needs. */
export function canRecord(): boolean {
  return (
    typeof navigator !== 'undefined' &&
    typeof navigator.mediaDevices?.getUserMedia === 'function' &&
    typeof AudioWorkletNode !== 'undefined'
  );
}

const CONSTRAINTS: MediaTrackConstraints = {
  echoCancellation: false,
  noiseSuppression: false,
  autoGainControl: false,
  channelCount: 1
};

export class MicRecorder {
  #workletUrl: string;
  #context: AudioContext | null = null;
  #stream: MediaStream | null = null;
  #source: MediaStreamAudioSourceNode | null = null;
  #node: AudioWorkletNode | null = null;
  #onChunk: ((chunk: RecorderChunk) => void) | null = null;

  constructor(workletUrl: string) {
    this.#workletUrl = workletUrl;
  }

  /**
   * Lists input devices for the picker.
   *
   * Device labels appear only after a permission grant, so a first call before
   * {@link MicRecorder.start} returns entries with empty labels; call it again
   * once recording has started to fill them in.
   */
  async listDevices(): Promise<RecorderDevice[]> {
    if (!navigator.mediaDevices?.enumerateDevices) return [];
    const devices = await navigator.mediaDevices.enumerateDevices();
    return devices
      .filter((device) => device.kind === 'audioinput')
      .map((device, index) => ({
        deviceId: device.deviceId,
        label: device.label || `Microphone ${index + 1}`
      }));
  }

  /**
   * Opens the device and starts capture, resolving with the true sample rate
   * and channel count once samples are flowing.
   */
  async start(options: StartOptions): Promise<{ sampleRate: number; channels: number }> {
    this.#onChunk = options.onChunk;
    const audio: MediaTrackConstraints = { ...CONSTRAINTS };
    if (options.deviceId) audio.deviceId = { exact: options.deviceId };
    this.#stream = await navigator.mediaDevices.getUserMedia({ audio });

    const context = new AudioContext();
    this.#context = context;
    await context.audioWorklet.addModule(this.#workletUrl);
    if (context.state === 'suspended') await context.resume();

    this.#source = context.createMediaStreamSource(this.#stream);
    const node = new AudioWorkletNode(context, 'recorder-processor', {
      numberOfInputs: 1,
      numberOfOutputs: 1,
      outputChannelCount: [1],
      channelCount: 1,
      channelCountMode: 'explicit',
      channelInterpretation: 'discrete',
      processorOptions: { channels: 1, batchMs: 100, levelMs: 33 }
    });
    this.#node = node;

    node.port.onmessage = (event: MessageEvent) => {
      const data = event.data;
      if (data.type === 'level') {
        options.onLevel({ rms: data.rms, peak: data.peak, clipped: data.clipped });
      } else if (data.type === 'chunk') {
        if (data.frames > 0) this.#onChunk?.({ samples: data.samples, frames: data.frames });
      }
    };

    this.#source.connect(node);
    // A silent output keeps the node pulled by the render graph without routing
    // the microphone back to the speakers.
    node.connect(context.destination);

    return { sampleRate: context.sampleRate, channels: 1 };
  }

  /**
   * Stops capture, flushing the tail of the buffer.
   *
   * Resolves after the worklet has posted its final chunk and this recorder has
   * forwarded it, so a caller that awaits `stop` before finishing the take never
   * drops the last fraction of a second.
   */
  async stop(): Promise<void> {
    const node = this.#node;
    if (!node) {
      this.#teardown();
      return;
    }
    await new Promise<void>((resolve) => {
      node.port.onmessage = (event: MessageEvent) => {
        const data = event.data;
        if (data.type === 'chunk') {
          if (data.frames > 0) this.#onChunk?.({ samples: data.samples, frames: data.frames });
          if (data.final) resolve();
        }
        // Level messages after stop are ignored.
      };
      node.port.postMessage({ command: 'stop' });
    });
    this.#teardown();
  }

  /** Stops capture and discards the take without a final flush. */
  cancel(): void {
    this.#teardown();
  }

  #teardown() {
    this.#node?.port.close();
    try {
      this.#source?.disconnect();
      this.#node?.disconnect();
    } catch {
      // Disconnecting an already-torn-down graph is not an error.
    }
    for (const track of this.#stream?.getTracks() ?? []) track.stop();
    void this.#context?.close();
    this.#node = null;
    this.#source = null;
    this.#stream = null;
    this.#context = null;
    this.#onChunk = null;
  }
}
