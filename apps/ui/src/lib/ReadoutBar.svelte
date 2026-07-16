<script lang="ts">
  import { formatTime, type Selection, type SelectionReadout } from './types';

  interface Props {
    selection: Selection;
    readout: SelectionReadout | null;
    formantMeans: number[] | null;
    showFormants: boolean;
    onPlay: () => void;
    onZoom: () => void;
    onVoiceReport: () => void;
    onClear: () => void;
  }

  let { selection, readout, formantMeans, showFormants, onPlay, onZoom, onVoiceReport, onClear }: Props =
    $props();

  function hz(value: number | null | undefined, digits = 0): string {
    return value === null || value === undefined || !Number.isFinite(value)
      ? '—'
      : `${value.toFixed(digits)} Hz`;
  }

  function db(value: number | null | undefined): string {
    return value === null || value === undefined || !Number.isFinite(value)
      ? '—'
      : `${value.toFixed(1)} dB`;
  }

  const visibleFormants = $derived(
    (formantMeans ?? []).slice(0, 3).map((value, index) => ({ index: index + 1, value }))
  );
</script>

<div
  class="readout"
  data-testid="readout-bar"
  data-t0={selection.t0}
  data-t1={selection.t1}
  data-f0={selection.f0}
  data-f1={selection.f1}
  data-band-energy={readout?.bandEnergyDb ?? ''}
>
  <div class="fields">
    <span class="field" data-testid="readout-duration">
      <span class="k">Δt</span><span class="v">{formatTime(selection.t1 - selection.t0)} s</span>
    </span>
    <span class="field">
      <span class="k">t</span><span class="v">{formatTime(selection.t0)}–{formatTime(selection.t1)}</span>
    </span>
    <span class="field">
      <span class="k">f</span>
      <span class="v">
        {#if selection.mode === 'box'}
          {selection.f0.toFixed(0)}–{selection.f1.toFixed(0)} Hz
        {:else}
          full band
        {/if}
      </span>
    </span>
    <span class="field" data-testid="readout-f0-mean" data-value={readout?.f0MeanHz ?? ''}>
      <span class="k">F0 mean</span><span class="v">{hz(readout?.f0MeanHz, 1)}</span>
    </span>
    <span class="field" data-testid="readout-f0-range">
      <span class="k">F0 min/max</span><span class="v">{hz(readout?.f0MinHz, 1)} / {hz(readout?.f0MaxHz, 1)}</span>
    </span>
    <span class="field" data-testid="readout-band-energy" data-value={readout?.bandEnergyDb ?? ''}>
      <span class="k">Band energy</span><span class="v">{db(readout?.bandEnergyDb)}</span>
    </span>
    <span class="field" data-testid="readout-intensity">
      <span class="k">Intensity</span><span class="v">{db(readout?.intensityMeanDb)}</span>
    </span>
    <span class="field" data-testid="readout-hnr">
      <span class="k">HNR</span><span class="v">{db(readout?.hnrMeanDb)}</span>
    </span>
    {#if showFormants}
      <span class="field provisional" data-testid="readout-formants" title="Formant tracking weights are provisional (T2.6 open)">
        <span class="k">F1–F3<sup>*</sup></span>
        <span class="v">
          {#each visibleFormants as formant (formant.index)}
            {hz(Number.isFinite(formant.value) ? formant.value : null, 0)}{formant.index < visibleFormants.length ? ' / ' : ''}
          {/each}
        </span>
      </span>
    {/if}
  </div>

  <div class="actions">
    {#if showFormants}
      <span class="marker" data-testid="provisional-marker">* provisional tracking</span>
    {/if}
    <button type="button" data-testid="selection-play" onclick={onPlay}>Play</button>
    <button type="button" data-testid="selection-zoom" onclick={onZoom}>Zoom to</button>
    <button type="button" data-testid="selection-voice-report" onclick={onVoiceReport}>Voice report</button>
    <button type="button" data-testid="selection-clear" onclick={onClear}>Clear</button>
  </div>
</div>

<style>
  .readout {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
    padding: 0.3rem 0.75rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--panel);
    color: var(--text);
    font-size: 0.78rem;
    font-variant-numeric: tabular-nums;
  }

  .fields {
    display: flex;
    align-items: center;
    gap: 0.9rem;
    flex-wrap: wrap;
    min-width: 0;
  }

  .field {
    display: inline-flex;
    align-items: baseline;
    gap: 0.35rem;
  }

  .field .k {
    color: var(--muted);
    font-size: 0.72rem;
  }

  .field .v {
    color: var(--text);
  }

  .provisional .v {
    color: var(--warn);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .marker {
    color: var(--warn);
    font-size: 0.72rem;
    margin-right: 0.3rem;
  }

  .actions button {
    border: 1px solid var(--chrome-strong);
    border-radius: 5px;
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.18rem 0.55rem;
    font-size: 0.76rem;
  }

  .actions button:hover {
    background: var(--panel);
  }
</style>
