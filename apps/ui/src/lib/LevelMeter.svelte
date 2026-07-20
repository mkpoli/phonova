<script lang="ts">
  import { amplitudeToDb, amplitudeToMeterFill } from './meter';
  import type { AudioId, CoreClientLike } from './types';

  interface Props {
    client: CoreClientLike | null;
    audioId: AudioId | null;
    duration: number;
    cursorTime: number;
    isPlaying: boolean;
  }

  let { client, audioId, duration, cursorTime, isPlaying }: Props = $props();

  // Neither `selectionReadout` nor `voiceReport` carries a peak/RMS field (the
  // engine's readout is analysis-parameter data, not a transport meter), so
  // this reads the running signal itself: a short window of exact samples
  // ending at the live cursor, refetched on a timer while transport plays.
  // `samplesInRange` already backs the waveform pane's high-zoom polyline, so
  // this adds no new engine surface, just a second consumer of it.
  const WINDOW_SECONDS = 0.09;
  const POLL_MS = 90;
  const PEAK_HOLD_MS = 900;
  const CLIP_THRESHOLD = 0.985;

  let level = $state<{ rms: number; peak: number } | null>(null);
  let peakHold = $state(0);
  let clipLatched = $state(false);
  let peakHoldTimer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    // A fresh recording drops any reading and warning from the last one.
    void audioId;
    level = null;
    peakHold = 0;
    clipLatched = false;
  });

  $effect(() => {
    if (!client || audioId === null || !isPlaying) {
      level = null;
      return;
    }
    const activeClient = client;
    const id = audioId;
    let cancelled = false;
    const poll = () => {
      const t1 = Math.min(duration, Math.max(WINDOW_SECONDS, cursorTime));
      const t0 = Math.max(0, t1 - WINDOW_SECONDS);
      activeClient
        .samplesInRange(id, t0, t1)
        .then((samples) => {
          if (cancelled || samples.length === 0) return;
          let sumSquares = 0;
          let peak = 0;
          for (let i = 0; i < samples.length; i += 1) {
            const v = samples[i];
            sumSquares += v * v;
            const abs = Math.abs(v);
            if (abs > peak) peak = abs;
          }
          level = { rms: Math.sqrt(sumSquares / samples.length), peak };
          if (peak > peakHold) {
            peakHold = peak;
            if (peakHoldTimer) clearTimeout(peakHoldTimer);
            peakHoldTimer = setTimeout(() => (peakHold = 0), PEAK_HOLD_MS);
          }
          if (peak >= CLIP_THRESHOLD) clipLatched = true;
        })
        .catch(() => {});
    };
    poll();
    const timer = setInterval(poll, POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  });

  const rmsFill = $derived(level ? amplitudeToMeterFill(level.rms) : 0);
  const peakFill = $derived(level ? amplitudeToMeterFill(level.peak) : 0);
  const peakHoldFill = $derived(amplitudeToMeterFill(peakHold));

  function dbLabel(value: number | null): string {
    if (value === null) return '—';
    const db = amplitudeToDb(value);
    return db <= -60 ? '-∞' : db.toFixed(1);
  }

  const TICKS = [0, -12, -24, -36, -48, -60];
  function tickTop(db: number): number {
    return ((0 - db) / 60) * 100;
  }
</script>

<aside class="meter" data-testid="level-meter" data-clipped={clipLatched} aria-label="Playback level">
  <h3 class="title">Level</h3>
  <div class="body">
    <div class="scale" aria-hidden="true">
      {#each TICKS as db (db)}
        <span class="tick" style:top={`${tickTop(db)}%`}>{db === 0 ? '0' : db}</span>
      {/each}
    </div>
    <div class="bar">
      <div class="fill" data-testid="level-meter-fill" style:height={`${rmsFill * 100}%`}></div>
      <div class="peak" data-testid="level-meter-peak" style:bottom={`${peakFill * 100}%`}></div>
      <div
        class="peak-hold"
        data-testid="level-meter-peak-hold"
        class:visible={peakHold > 0}
        style:bottom={`${peakHoldFill * 100}%`}
      ></div>
    </div>
  </div>
  <dl class="values">
    <div>
      <dt>Peak</dt>
      <dd data-testid="level-meter-peak-value">{dbLabel(level?.peak ?? null)}</dd>
    </div>
    <div>
      <dt>RMS</dt>
      <dd data-testid="level-meter-rms-value">{dbLabel(level?.rms ?? null)}</dd>
    </div>
  </dl>
  <button
    type="button"
    class="clip"
    class:hot={clipLatched}
    data-testid="level-meter-clip"
    aria-pressed={clipLatched}
    title={clipLatched ? 'Clipped during playback — click to clear' : 'No clipping'}
    onclick={() => (clipLatched = false)}
  >
    Clip
  </button>
</aside>

<style>
  /* A thin, quiet column: the meter reports level without competing with the
     spectrogram for attention (DESIGN.md — beauty through material honesty).
     Its gradient stays in the accent color through most of the range and only
     turns to warn/danger colors near the top few dB, so it reads as calm until
     the signal is actually close to clipping. */
  .meter {
    width: 3.75rem;
    min-width: 3.75rem;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    overflow-y: auto;
    padding: 0.6rem 0.3rem 0.75rem;
    border-left: 1px solid var(--chrome-strong);
    background: var(--panel);
    color: var(--muted);
  }

  .title {
    margin: 0 0 0.5rem;
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .body {
    flex: none;
    display: flex;
    align-items: stretch;
    gap: 0.3rem;
    height: 11rem;
  }

  .scale {
    position: relative;
    width: 1.15rem;
    flex: none;
  }

  .tick {
    position: absolute;
    right: 0;
    transform: translateY(-50%);
    font-size: 0.56rem;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
    opacity: 0.75;
  }

  .bar {
    position: relative;
    width: 0.5rem;
    flex: none;
    border-radius: 3px;
    background: var(--panel-soft);
    border: 1px solid var(--chrome-strong);
    overflow: hidden;
  }

  .fill {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    border-radius: 3px;
    background: linear-gradient(
      to top,
      var(--accent) 0%,
      var(--accent) 65%,
      var(--warn) 86%,
      var(--danger) 100%
    );
    transition: height 110ms linear;
  }

  .peak {
    position: absolute;
    left: 0;
    right: 0;
    height: 1px;
    background: var(--text);
    opacity: 0.55;
    transition: bottom 110ms linear;
  }

  .peak-hold {
    position: absolute;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--text);
    opacity: 0;
    transition: bottom 110ms linear;
  }

  .peak-hold.visible {
    opacity: 0.9;
  }

  .values {
    flex: none;
    margin: 0.6rem 0 0;
    text-align: center;
    font-size: 0.62rem;
    font-variant-numeric: tabular-nums;
    line-height: 1.5;
  }

  .values div {
    display: flex;
    flex-direction: column;
  }

  .values dt {
    color: var(--muted);
    font-size: 0.58rem;
    letter-spacing: 0.03em;
    text-transform: uppercase;
  }

  .values dd {
    margin: 0;
    color: var(--text);
    font-size: 0.72rem;
  }

  .clip {
    flex: none;
    margin-top: 0.55rem;
    padding: 0.15rem 0.4rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted);
    font-size: 0.58rem;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    transition:
      background var(--t-fast),
      border-color var(--t-fast),
      color var(--t-fast);
  }

  .clip.hot {
    border-color: var(--danger);
    background: color-mix(in oklab, var(--danger) 16%, transparent);
    color: var(--danger);
  }
</style>
