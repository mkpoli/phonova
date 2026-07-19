<script lang="ts">
  import IconSliders from '~icons/lucide/sliders-horizontal';
  import IconX from '~icons/lucide/x';
  import type { OverlayParams, OverlayStats } from './types';

  interface Props {
    params: OverlayParams;
    stats: OverlayStats;
    onClose?: () => void;
  }

  let { params, stats, onClose }: Props = $props();

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
</script>

<aside class="inspector" data-testid="inspector" aria-label="Analysis inspector">
  <header class="head">
    <h2><IconSliders aria-hidden="true" />Inspector</h2>
    {#if onClose}
      <button type="button" class="close" aria-label="Close inspector" onclick={onClose}>
        <IconX aria-hidden="true" />
      </button>
    {/if}
  </header>

  <section class="group" data-testid="inspector-pitch">
    <div class="group-head">
      <label class="toggle">
        <input type="checkbox" data-testid="toggle-pitch" bind:checked={params.pitch.show} />
        <span class="swatch pitch"></span>
        <span>Pitch</span>
      </label>
    </div>
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
  </section>

  <section class="group" data-testid="inspector-formant">
    <div class="group-head">
      <label class="toggle">
        <input type="checkbox" data-testid="toggle-formant" bind:checked={params.formant.show} />
        <span class="swatch formant"></span>
        <span>Formants</span>
      </label>
    </div>
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
        <option value="track">Connected tracks</option>
      </select>
      <p class="note">
        Speckles are the Praat-familiar dot-per-candidate view, sized by bandwidth. Tracks connect
        each formant across time and break wherever a frame has no candidate for it, rather than
        drawing through the gap.
      </p>
    </div>
  </section>

  <section class="group" data-testid="inspector-intensity">
    <div class="group-head">
      <label class="toggle">
        <input type="checkbox" data-testid="toggle-intensity" bind:checked={params.intensity.show} />
        <span class="swatch intensity"></span>
        <span>Intensity</span>
      </label>
    </div>
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
    justify-content: space-between;
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

  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
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

  .group {
    padding: 0.6rem 0;
    border-top: 1px solid var(--chrome-strong);
  }

  .group-head {
    margin-bottom: 0.4rem;
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    font-weight: 600;
  }

  .toggle.inline {
    font-weight: 500;
  }

  .swatch {
    width: 0.85rem;
    height: 0.85rem;
    border-radius: 2px;
    box-shadow: 0 0 0 1px rgba(4, 8, 16, 0.5);
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
