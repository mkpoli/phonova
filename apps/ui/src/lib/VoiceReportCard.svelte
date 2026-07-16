<script lang="ts">
  import { formatTime, type VoiceReportData } from './types';

  interface Props {
    report: VoiceReportData | null;
    loading: boolean;
    onClose: () => void;
  }

  let { report, loading, onClose }: Props = $props();
  let copied = $state(false);

  function num(value: number | null | undefined, digits: number, unit = ''): string {
    return value === null || value === undefined || !Number.isFinite(value)
      ? '—'
      : `${value.toFixed(digits)}${unit}`;
  }

  function pct(value: number | null | undefined): string {
    return value === null || value === undefined || !Number.isFinite(value)
      ? '—'
      : `${(value * 100).toFixed(3)} %`;
  }

  // Flat measure rows drive both the table and the CSV export, so copied text
  // matches what the card shows.
  const rows = $derived.by<[string, string][]>(() => {
    if (!report) return [];
    return [
      ['Mean F0', num(report.pitch.meanHz, 2, ' Hz')],
      ['Median F0', num(report.pitch.medianHz, 2, ' Hz')],
      ['Min F0', num(report.pitch.minHz, 2, ' Hz')],
      ['Max F0', num(report.pitch.maxHz, 2, ' Hz')],
      ['Jitter (local)', pct(report.jitter.local)],
      ['Jitter (local, abs)', num(report.jitter.localAbsolute ? report.jitter.localAbsolute * 1e6 : report.jitter.localAbsolute, 2, ' µs')],
      ['Jitter (rap)', pct(report.jitter.rap)],
      ['Jitter (ppq5)', pct(report.jitter.ppq5)],
      ['Jitter (ddp)', pct(report.jitter.ddp)],
      ['Shimmer (local)', pct(report.shimmer.local)],
      ['Shimmer (local, dB)', num(report.shimmer.localDb, 3, ' dB')],
      ['Shimmer (apq3)', pct(report.shimmer.apq3)],
      ['Shimmer (apq5)', pct(report.shimmer.apq5)],
      ['Shimmer (apq11)', pct(report.shimmer.apq11)],
      ['Shimmer (dda)', pct(report.shimmer.dda)],
      ['Mean HNR', num(report.meanHnrDb, 2, ' dB')],
      ['CPP', num(report.cppDb, 2, ' dB')],
      ['CPPS', num(report.cppsDb, 2, ' dB')],
      ['Voice breaks', `${report.voiceBreaks.count} (${num(report.voiceBreaks.totalSeconds, 3, ' s')})`],
      ['Spectral CoG', num(report.moments.centreOfGravityHz, 1, ' Hz')],
      ['Spectral SD', num(report.moments.standardDeviationHz, 1, ' Hz')],
      ['Spectral skewness', num(report.moments.skewness, 3)],
      ['Spectral kurtosis', num(report.moments.kurtosis, 3)],
      ['Pulses', String(report.pulseCount)]
    ];
  });

  async function copyCsv() {
    if (!report) return;
    const lines = ['measure,value', ...rows.map(([k, v]) => `${k},${v}`)];
    try {
      await navigator.clipboard.writeText(lines.join('\n'));
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      copied = false;
    }
  }
</script>

<div class="backdrop" data-testid="voice-report-card">
  <div class="card" role="dialog" aria-modal="true" aria-label="Voice report">
    <header>
      <h2>Voice report</h2>
      {#if report}
        <span class="span">{formatTime(report.t0)}–{formatTime(report.t1)} s</span>
      {/if}
      <button type="button" class="close" data-testid="voice-report-close" onclick={onClose} aria-label="Close">×</button>
    </header>

    {#if loading}
      <p class="status" data-testid="voice-report-loading">Measuring…</p>
    {:else if report}
      <div class="grid" data-testid="voice-report-values"
        data-jitter-local={report.jitter.local ?? ''}
        data-shimmer-local={report.shimmer.local ?? ''}
        data-hnr={report.meanHnrDb ?? ''}
      >
        {#each rows as [key, value] (key)}
          <div class="row">
            <span class="rk">{key}</span>
            <span class="rv">{value}</span>
          </div>
        {/each}
      </div>

      <footer>
        <p class="params">
          Pitch floor {report.params.pitchFloorHz.toFixed(0)} Hz · ceiling
          {report.params.pitchCeilingHz.toFixed(0)} Hz · HNR window
          {report.params.periodsPerWindow.toFixed(1)} periods · CPP frame
          {(report.params.cppFrameLengthSeconds * 1000).toFixed(0)} ms
          ({report.params.cppMinF0Hz.toFixed(0)}–{report.params.cppMaxF0Hz.toFixed(0)} Hz)
        </p>
        <button type="button" class="copy" data-testid="voice-report-copy" onclick={copyCsv}>
          {copied ? 'Copied' : 'Copy CSV'}
        </button>
      </footer>
    {:else}
      <p class="status">No measurement.</p>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(15, 23, 42, 0.42);
    z-index: 30;
  }

  .card {
    width: min(34rem, calc(100vw - 2rem));
    max-height: calc(100vh - 3rem);
    overflow: auto;
    padding: 1rem 1.2rem 1.2rem;
    border: 1px solid var(--chrome-strong);
    border-radius: 12px;
    background: var(--panel);
    color: var(--text);
    box-shadow: 0 24px 60px rgba(15, 23, 42, 0.3);
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    margin-bottom: 0.75rem;
  }

  header h2 {
    margin: 0;
    font-size: 1.02rem;
  }

  .span {
    color: var(--muted);
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
  }

  .close {
    margin-left: auto;
    border: 1px solid var(--chrome-strong);
    border-radius: 6px;
    background: var(--panel-soft);
    color: var(--text);
    width: 1.7rem;
    height: 1.7rem;
    font-size: 1rem;
    line-height: 1;
  }

  .status {
    margin: 0.5rem 0;
    color: var(--muted);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.2rem 1.2rem;
    font-size: 0.82rem;
    font-variant-numeric: tabular-nums;
  }

  .row {
    display: flex;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.16rem 0;
    border-bottom: 1px solid color-mix(in oklab, var(--chrome-strong) 55%, transparent);
  }

  .rk {
    color: var(--muted);
  }

  footer {
    margin-top: 0.9rem;
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 1rem;
  }

  .params {
    margin: 0;
    color: var(--muted);
    font-size: 0.74rem;
    line-height: 1.5;
  }

  .copy {
    flex: none;
    border: 1px solid var(--chrome-strong);
    border-radius: 6px;
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.3rem 0.7rem;
    font-size: 0.78rem;
  }
</style>
