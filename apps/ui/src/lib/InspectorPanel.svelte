<script lang="ts">
  import IconLayers from '~icons/lucide/layers';
  import IconEye from '~icons/lucide/eye';
  import IconEyeOff from '~icons/lucide/eye-off';
  import IconChevronRight from '~icons/lucide/chevron-right';
  import IconX from '~icons/lucide/x';
  import type { OverlayParams, OverlayStats, SelectionReadout } from './types';

  interface Props {
    params: OverlayParams;
    stats: OverlayStats;
    /** Selection readout, for each layer's live value. Null with no selection. */
    readout?: SelectionReadout | null;
    /** Provisional tracked-formant means over the selection (F1, F2, …). */
    formantMeans?: number[] | null;
    onClose?: () => void;
  }

  let { params, stats, readout = null, formantMeans = null, onClose }: Props = $props();

  // A ceiling clips the data once a tracked value crowds it. Tracked maxima
  // sit a little under the ceiling even when the true value is cut off, so the
  // badge fires within 5% of the ceiling (ux.md: values crowding the ceiling
  // get a warning badge).
  let pitchClipped = $derived(
    stats.pitchMaxHz > 0 && stats.pitchMaxHz >= params.pitch.ceilingHz * 0.95
  );
  let formantClipped = $derived(
    stats.formantMaxHz > 0 && stats.formantMaxHz >= params.formant.ceilingHz * 0.95
  );

  // A connected track asserts that consecutive candidates are the same
  // formant. Only the Viterbi-smoothed track carries that assignment; raw
  // Burg candidates are just "N loudest resonances this frame" with no
  // cross-frame identity, so drawing a line through them would claim a
  // tracking decision nothing made. Turning smoothing off while a track is
  // selected falls back to speckles rather than silently rendering a track
  // the data no longer supports.
  $effect(() => {
    if (!params.formant.smoothed && params.formant.mark === 'track') {
      params.formant.mark = 'speckle';
    }
  });

  let expanded = $state({ pitch: true, formant: true, intensity: true });

  let visibleCount = $derived(
    Number(params.pitch.show) + Number(params.formant.show) + Number(params.intensity.show)
  );

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

  // Raw Burg candidates carry no cross-frame identity (that is exactly why
  // speckles, not a line, are the default mark), so there is no single
  // current F1/F2 to report unless the smoothed track supplies one.
  let formantLive = $derived(
    formantMeans && formantMeans.length >= 2
      ? `${formantMeans[0].toFixed(0)} · ${formantMeans[1].toFixed(0)} Hz`
      : '—'
  );
</script>

<aside class="inspector" data-testid="inspector" aria-label="Analysis layers">
  <header class="head">
    <h2><IconLayers aria-hidden="true" />Layers</h2>
    <span class="count">{visibleCount}/3 visible</span>
    {#if onClose}
      <button type="button" class="close" aria-label="Close inspector" onclick={onClose}>
        <IconX aria-hidden="true" />
      </button>
    {/if}
  </header>

  <section class="layer" class:off={!params.pitch.show} data-testid="inspector-pitch">
    <div class="layer-head">
      <button
        type="button"
        class="eye"
        data-testid="toggle-pitch"
        aria-pressed={params.pitch.show}
        aria-label={params.pitch.show ? 'Hide pitch' : 'Show pitch'}
        title={params.pitch.show ? 'Hide pitch' : 'Show pitch'}
        onclick={() => (params.pitch.show = !params.pitch.show)}
      >
        {#if params.pitch.show}<IconEye aria-hidden="true" />{:else}<IconEyeOff aria-hidden="true" />{/if}
      </button>
      <span class="swatch pitch"></span>
      <button
        type="button"
        class="layer-name"
        aria-expanded={expanded.pitch}
        onclick={() => (expanded.pitch = !expanded.pitch)}
      >
        <span class="chev" class:open={expanded.pitch}><IconChevronRight aria-hidden="true" /></span>
        Pitch
      </button>
      <span class="live-value">{hz(readout?.f0MeanHz, 1)}</span>
    </div>
    {#if expanded.pitch}
      <div class="params">
        <div class="field">
          <div class="label-row"><span>Floor</span><span class="unit">Hz</span></div>
          <input
            type="number"
            min="20"
            max="600"
            step="5"
            data-testid="pitch-floor"
            bind:value={params.pitch.floorHz}
          />
          <p class="note">Default 75 Hz — Praat raw-autocorrelation floor.</p>
        </div>
        <div class="field">
          <div class="label-row">
            <span>Ceiling</span>
            {#if pitchClipped}
              <span class="badge" data-testid="pitch-clip-badge" title="Tracked pitch reaches the ceiling"
                >clips ~{Math.round(stats.pitchMaxHz)} Hz</span
              >
            {:else}
              <span class="unit">Hz</span>
            {/if}
          </div>
          <input
            type="number"
            min="100"
            max="2000"
            step="10"
            data-testid="pitch-ceiling"
            bind:value={params.pitch.ceilingHz}
          />
          <p class="note">Default 600 Hz — Praat. Lower toward 300 Hz for male speech.</p>
        </div>
      </div>
    {/if}
  </section>

  <section class="layer" class:off={!params.formant.show} data-testid="inspector-formant">
    <div class="layer-head">
      <button
        type="button"
        class="eye"
        data-testid="toggle-formant"
        aria-pressed={params.formant.show}
        aria-label={params.formant.show ? 'Hide formants' : 'Show formants'}
        title={params.formant.show ? 'Hide formants' : 'Show formants'}
        onclick={() => (params.formant.show = !params.formant.show)}
      >
        {#if params.formant.show}<IconEye aria-hidden="true" />{:else}<IconEyeOff aria-hidden="true" />{/if}
      </button>
      <span class="swatch formant"></span>
      <button
        type="button"
        class="layer-name"
        aria-expanded={expanded.formant}
        onclick={() => (expanded.formant = !expanded.formant)}
      >
        <span class="chev" class:open={expanded.formant}><IconChevronRight aria-hidden="true" /></span>
        Formants
      </button>
      <span class="live-value">{formantLive}</span>
    </div>
    {#if expanded.formant}
      <div class="params">
        <div class="field">
          <div class="label-row">
            <span>Ceiling</span>
            {#if formantClipped}
              <span class="badge" data-testid="formant-clip-badge" title="A tracked formant reaches the ceiling"
                >clips ~{Math.round(stats.formantMaxHz)} Hz</span
              >
            {:else}
              <span class="unit">Hz</span>
            {/if}
          </div>
          <input
            type="number"
            min="1000"
            max="12000"
            step="100"
            data-testid="formant-ceiling"
            bind:value={params.formant.ceilingHz}
          />
          <p class="note">Default 5500 Hz — Praat adult-female. Use 5000 Hz for adult male.</p>
        </div>
        <div class="field">
          <div class="label-row"><span>Max formants</span></div>
          <input
            type="number"
            min="1"
            max="7"
            step="1"
            data-testid="formant-count"
            bind:value={params.formant.maxFormants}
          />
          <p class="note">Default 5 — Praat.</p>
        </div>
        <div class="field">
          <label class="toggle inline">
            <input type="checkbox" data-testid="formant-smoothed" bind:checked={params.formant.smoothed} />
            <span>Tracked (provisional)</span>
          </label>
          <p class="note">
            Off shows raw Burg candidates. On runs Xia–Espy-Wilson smoothing, whose weights stay
            provisional until gate review.
          </p>
        </div>
        <div class="field">
          <div class="label-row"><span>Mark</span></div>
          <select data-testid="formant-mark" bind:value={params.formant.mark}>
            <option value="speckle">Speckles</option>
            <option value="track" disabled={!params.formant.smoothed}>Connected tracks</option>
          </select>
          {#if params.formant.smoothed}
            <p class="note">
              Speckles are the Praat-familiar dot-per-candidate view, sized by bandwidth. Tracks
              connect each formant across time and break wherever a frame has no candidate for it,
              rather than drawing through the gap.
            </p>
          {:else}
            <p class="note">
              Connected tracks needs Tracked (provisional) on: raw candidates carry no identity
              from one frame to the next, so a line through them would join points that were never
              the same formant.
            </p>
          {/if}
        </div>
      </div>
    {/if}
  </section>

  <section class="layer" class:off={!params.intensity.show} data-testid="inspector-intensity">
    <div class="layer-head">
      <button
        type="button"
        class="eye"
        data-testid="toggle-intensity"
        aria-pressed={params.intensity.show}
        aria-label={params.intensity.show ? 'Hide intensity' : 'Show intensity'}
        title={params.intensity.show ? 'Hide intensity' : 'Show intensity'}
        onclick={() => (params.intensity.show = !params.intensity.show)}
      >
        {#if params.intensity.show}<IconEye aria-hidden="true" />{:else}<IconEyeOff aria-hidden="true" />{/if}
      </button>
      <span class="swatch intensity"></span>
      <button
        type="button"
        class="layer-name"
        aria-expanded={expanded.intensity}
        onclick={() => (expanded.intensity = !expanded.intensity)}
      >
        <span class="chev" class:open={expanded.intensity}><IconChevronRight aria-hidden="true" /></span>
        Intensity
      </button>
      <span class="live-value">{db(readout?.intensityMeanDb)}</span>
    </div>
    {#if expanded.intensity}
      <div class="params">
        <div class="field">
          <div class="label-row"><span>Floor</span><span class="unit">Hz</span></div>
          <input
            type="number"
            min="20"
            max="400"
            step="10"
            data-testid="intensity-floor"
            bind:value={params.intensity.floorHz}
          />
          <p class="note">Default 100 Hz — Praat pitch floor sets the intensity window length.</p>
        </div>
      </div>
    {/if}
  </section>
</aside>

<style>
  .inspector {
    width: 17rem;
    min-width: 17rem;
    height: 100%;
    overflow-y: auto;
    border-left: 1px solid var(--chrome-strong);
    background: var(--panel);
    color: var(--text);
    padding: 0.75rem 0.85rem 1.5rem;
    font-size: 0.85rem;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin: -0.75rem -0.85rem 0.75rem;
    padding: 0.6rem 0.85rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--panel-soft);
  }

  .head h2 {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.95rem;
    font-weight: 600;
  }

  .head h2 :global(svg) {
    font-size: 1rem;
    color: var(--muted);
  }

  .head .count {
    color: var(--muted);
    font-size: 0.7rem;
    font-variant-numeric: tabular-nums;
  }

  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    margin-left: auto;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted);
    font-size: 1rem;
    line-height: 1;
    padding: 0.2rem;
    transition:
      background var(--t-fast),
      color var(--t-fast);
  }

  .close:hover {
    background: var(--panel-soft);
    color: var(--text);
  }

  .layer {
    padding: 0.5rem 0;
    border-top: 1px solid var(--chrome-strong);
  }

  .layer-head {
    display: flex;
    align-items: center;
    gap: 0.45rem;
  }

  .eye {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: 1.6rem;
    height: 1.6rem;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--accent);
    transition:
      background var(--t-fast),
      color var(--t-fast);
  }

  .eye:hover {
    background: var(--accent-tint);
  }

  .layer.off .eye {
    color: var(--muted);
  }

  .swatch {
    flex: none;
    width: 0.6rem;
    height: 0.6rem;
    border-radius: 2px;
    box-shadow: 0 0 0 1px rgba(4, 8, 16, 0.5);
  }

  .layer.off .swatch {
    opacity: 0.35;
  }

  .swatch.pitch {
    background: var(--overlay-pitch);
  }

  .swatch.formant {
    background: var(--overlay-formant);
  }

  .swatch.intensity {
    background: var(--overlay-intensity);
  }

  .layer-name {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    flex: 1;
    min-width: 0;
    border: none;
    background: transparent;
    color: var(--text);
    font-weight: 600;
    text-align: left;
    padding: 0;
  }

  .layer.off .layer-name {
    color: var(--muted);
  }

  .chev {
    display: inline-flex;
    flex: none;
    color: var(--muted);
    transition: transform var(--t-fast);
  }

  .chev.open {
    transform: rotate(90deg);
  }

  .live-value {
    flex: none;
    color: var(--text);
    font-size: 0.78rem;
    font-variant-numeric: tabular-nums;
  }

  .layer.off .live-value {
    color: var(--muted);
  }

  .params {
    padding-left: 2.05rem;
  }

  .toggle.inline {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    font-weight: 500;
  }

  .field {
    margin: 0.5rem 0;
  }

  .label-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.2rem;
  }

  .unit {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  input[type='number'] {
    width: 100%;
    min-height: 2rem;
    padding: 0.32rem 0.45rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-sm);
    background: var(--panel-soft);
    color: var(--text);
    font-variant-numeric: tabular-nums;
    transition:
      border-color var(--t-fast),
      box-shadow var(--t-fast);
  }

  input[type='number']:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--accent) 20%, transparent);
  }

  select {
    width: 100%;
    min-height: 2rem;
    padding: 0.32rem 0.45rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-sm);
    background: var(--panel-soft);
    color: var(--text);
    font: inherit;
    transition:
      border-color var(--t-fast),
      box-shadow var(--t-fast);
  }

  select:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--accent) 20%, transparent);
  }

  .note {
    margin: 0.28rem 0 0;
    color: var(--muted);
    font-size: 0.72rem;
    line-height: 1.35;
  }

  .badge {
    padding: 0.05rem 0.4rem;
    border-radius: 999px;
    background: color-mix(in oklab, var(--warn), transparent 78%);
    color: var(--warn);
    font-size: 0.7rem;
    font-weight: 600;
  }
</style>
