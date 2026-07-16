<script lang="ts">
  import { formatTime } from './types';

  interface RecorderDevice {
    deviceId: string;
    label: string;
  }

  interface Props {
    devices: RecorderDevice[];
    selectedDeviceId: string;
    /** Meter reading, RMS and peak in [0, 1]. */
    level: { rms: number; peak: number; clipped: boolean };
    /** Whether the take has clipped at any point (a latched warning). */
    clipLatched: boolean;
    elapsedSeconds: number;
    /** True capture rate in hertz, or 0 before the device reports it. */
    sampleRate: number;
    onSelectDevice: (deviceId: string) => void;
    onStop: () => void;
    onCancel: () => void;
  }

  let {
    devices,
    selectedDeviceId,
    level,
    clipLatched,
    elapsedSeconds,
    sampleRate,
    onSelectDevice,
    onStop,
    onCancel
  }: Props = $props();

  // A meter fill that reads sensibly for speech: map the level onto a decibel
  // scale over the top 60 dB so quiet passages still move the bar.
  function meterFill(value: number): number {
    if (value <= 0) return 0;
    const db = 20 * Math.log10(value);
    return Math.max(0, Math.min(1, (db + 60) / 60));
  }

  const rmsFill = $derived(meterFill(level.rms));
  const peakFill = $derived(meterFill(level.peak));

  function rateLabel(hz: number): string {
    if (hz <= 0) return '—';
    return hz % 1000 === 0 ? `${hz / 1000} kHz` : `${(hz / 1000).toFixed(1)} kHz`;
  }
</script>

<div class="strip" data-testid="recording-strip" role="region" aria-label="Recording">
  <div class="pulse" aria-hidden="true"></div>
  <span class="label">Recording</span>

  <label class="device">
    <span class="device-label">Input</span>
    <select
      data-testid="recording-device"
      aria-label="Input device"
      value={selectedDeviceId}
      onchange={(event) => onSelectDevice(event.currentTarget.value)}
    >
      {#if devices.length === 0}
        <option value="">Default microphone</option>
      {/if}
      {#each devices as device (device.deviceId)}
        <option value={device.deviceId}>{device.label}</option>
      {/each}
    </select>
  </label>

  <div class="meter" data-testid="recording-level" title="Input level">
    <div class="meter-track">
      <div class="meter-rms" style={`width: ${(rmsFill * 100).toFixed(1)}%`}></div>
      <div class="meter-peak" style={`left: ${(peakFill * 100).toFixed(1)}%`}></div>
    </div>
    <span
      class="clip"
      class:on={clipLatched}
      data-testid="recording-clip"
      data-clipped={clipLatched}
      aria-hidden={!clipLatched}
    >
      CLIP
    </span>
  </div>

  <span class="elapsed" data-testid="recording-elapsed">{formatTime(elapsedSeconds)}</span>
  <span class="rate" data-testid="recording-samplerate">{rateLabel(sampleRate)}</span>

  <div class="actions">
    <button type="button" class="cancel" data-testid="recording-cancel" onclick={onCancel}>
      Cancel
    </button>
    <button type="button" class="stop" data-testid="recording-stop" onclick={onStop}>Stop</button>
  </div>
</div>

<style>
  .strip {
    position: fixed;
    left: 50%;
    bottom: 1rem;
    transform: translateX(-50%);
    z-index: 15;
    display: flex;
    align-items: center;
    gap: 0.9rem;
    max-width: calc(100vw - 2rem);
    padding: 0.55rem 0.9rem;
    border: 1px solid var(--chrome-strong);
    border-radius: 10px;
    background: var(--panel);
    color: var(--text);
    box-shadow: 0 16px 40px rgba(15, 23, 42, 0.28);
    font-size: 0.85rem;
  }

  .pulse {
    width: 0.7rem;
    height: 0.7rem;
    border-radius: 50%;
    background: #dc2626;
    animation: blink 1.1s ease-in-out infinite;
  }

  @keyframes blink {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.35;
    }
  }

  .label {
    font-weight: 600;
    letter-spacing: 0.01em;
  }

  .device {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  .device-label {
    color: var(--muted);
    font-size: 0.78rem;
  }

  .device select {
    max-width: 12rem;
    border: 1px solid var(--chrome-strong);
    border-radius: 6px;
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.2rem 0.4rem;
  }

  .meter {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .meter-track {
    position: relative;
    width: 9rem;
    height: 0.7rem;
    border-radius: 4px;
    background: var(--panel-soft);
    border: 1px solid var(--chrome-strong);
    overflow: hidden;
  }

  .meter-rms {
    position: absolute;
    inset: 0 auto 0 0;
    background: linear-gradient(90deg, var(--accent), var(--accent-strong));
    transition: width 60ms linear;
  }

  .meter-peak {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--text);
    transition: left 60ms linear;
  }

  .clip {
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: var(--muted);
    opacity: 0.35;
  }

  .clip.on {
    color: #fff;
    opacity: 1;
    background: #dc2626;
    padding: 0.05rem 0.3rem;
    border-radius: 4px;
    animation: flash 0.5s ease-in-out 3;
  }

  @keyframes flash {
    0%,
    100% {
      background: #dc2626;
    }
    50% {
      background: #f87171;
    }
  }

  .elapsed {
    font-variant-numeric: tabular-nums;
    font-weight: 600;
    min-width: 3.5rem;
    text-align: right;
  }

  .rate {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .actions {
    display: flex;
    gap: 0.4rem;
  }

  .actions button {
    border: 1px solid var(--chrome-strong);
    border-radius: 6px;
    padding: 0.3rem 0.75rem;
    background: var(--panel-soft);
    color: var(--text);
    font-size: 0.82rem;
  }

  .actions .stop {
    border-color: var(--accent);
    background: color-mix(in oklab, var(--accent) 22%, var(--panel-soft));
    font-weight: 600;
  }

  .actions button:hover {
    filter: brightness(1.05);
  }
</style>
