<script lang="ts">
  import IconRotateCcw from '~icons/lucide/rotate-ccw';
  import type { ViewportState } from './types';

  interface Props {
    viewport: ViewportState;
    /** Default ceiling; the reset chip shows whenever the live ceiling departs it. */
    defaultCeiling?: number;
    /** Multiplies the frequency ceiling by `factor`; the caller clamps the range. */
    onScale?: (factor: number) => void;
    /** Restores the ceiling to {@link defaultCeiling}. */
    onReset?: () => void;
  }

  let { viewport, defaultCeiling = 5000, onScale, onReset }: Props = $props();

  let rulerEl = $state<HTMLDivElement | null>(null);
  let rulerHeight = $state(1);
  let dragging = $state(false);
  let lastY = 0;

  // Height of the top-right units chip in device-independent pixels; the max
  // tick label and the pitch ceiling label both step below this band so the
  // corner reads as one clean cell — units chip on top, the highest label under
  // it, nothing stacked.
  const CORNER_CHIP_PX = 22;

  const scaled = $derived(Math.abs(viewport.f1 - defaultCeiling) > 1);

  $effect(() => {
    if (!rulerEl) return;
    const observer = new ResizeObserver(() => {
      rulerHeight = rulerEl?.clientHeight ?? 1;
    });
    observer.observe(rulerEl);
    rulerHeight = rulerEl.clientHeight || 1;
    return () => observer.disconnect();
  });

  function niceStep(totalRange: number, pixelSpan: number, targetPx: number): number {
    if (totalRange <= 0 || pixelSpan <= 0) return 1;
    const raw = (totalRange / pixelSpan) * targetPx;
    const mag = Math.pow(10, Math.floor(Math.log10(raw)));
    const norm = raw / mag;
    const step = norm < 1.5 ? 1 : norm < 3.5 ? 2 : norm < 7.5 ? 5 : 10;
    return step * mag;
  }

  function tickDecimals(step: number): number {
    return Math.max(0, Math.min(6, -Math.floor(Math.log10(step))));
  }

  const majorStep = $derived(niceStep(viewport.f1 - viewport.f0, rulerHeight, 100));
  const decimals = $derived(tickDecimals(majorStep));

  const majorTicks = $derived.by(() => {
    if (viewport.f1 - viewport.f0 <= 0) return [];
    const ticks: number[] = [];
    for (let f = Math.ceil(viewport.f0 / majorStep) * majorStep; f <= viewport.f1; f += majorStep) {
      ticks.push(f);
    }
    return ticks;
  });

  function pct(freq: number): number {
    return (1 - (freq - viewport.f0) / (viewport.f1 - viewport.f0)) * 100;
  }

  // Pixels the label must drop to clear the corner units chip; the dash stays at
  // the true frequency, only the number steps down.
  function labelShift(freq: number): number {
    const topPx = (pct(freq) / 100) * rulerHeight;
    return Math.max(0, CORNER_CHIP_PX - topPx);
  }

  function onPointerDown(event: PointerEvent) {
    if (event.button !== 0 || !onScale) return;
    event.stopPropagation();
    event.preventDefault();
    dragging = true;
    lastY = event.clientY;
    rulerEl?.setPointerCapture(event.pointerId);
  }

  function onPointerMove(event: PointerEvent) {
    if (!dragging || !onScale) return;
    const delta = event.clientY - lastY;
    lastY = event.clientY;
    if (delta === 0) return;
    // Drag down lowers the ceiling (zoom into lower frequencies); drag up raises
    // it. Exponential so the gesture feels uniform across the range.
    onScale(Math.exp(-delta * 0.006));
  }

  function onPointerUp(event: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    rulerEl?.releasePointerCapture(event.pointerId);
  }
</script>

<div
  class="ruler"
  class:dragging
  data-testid="frequency-ruler"
  bind:this={rulerEl}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  role="slider"
  aria-label="Spectrogram frequency ceiling"
  aria-valuenow={Math.round(viewport.f1)}
  tabindex="-1"
>
  <span class="units" data-testid="frequency-units">Hz</span>
  {#each majorTicks as f (f)}
    <div class="tick" style="top: {pct(f)}%">
      <span class="label" style="transform: translateY({labelShift(f)}px)">{f.toFixed(decimals)}</span>
      <span class="dash"></span>
    </div>
  {/each}
  {#if scaled}
    <button
      type="button"
      class="reset"
      data-testid="frequency-reset"
      title="Reset frequency ceiling"
      onpointerdown={(event) => event.stopPropagation()}
      onclick={() => onReset?.()}
    >
      <IconRotateCcw aria-hidden="true" />
      <span>Reset</span>
    </button>
  {/if}
</div>

<style>
  .ruler {
    position: absolute;
    top: 0;
    bottom: 0;
    right: 0;
    /* Wide enough for a full 4-digit chip (the pane spans up to 5000 Hz by
       default) with room to spare, and padding-right below clears a lane for
       the frozen pitch-axis numbers, which stay flush at the true edge. */
    width: 4.5rem;
    overflow: hidden;
    /* The ruler is a drag surface for the frequency ceiling, so it sits above
       the selection layer and takes the pointer along its strip. */
    pointer-events: auto;
    cursor: ns-resize;
    touch-action: none;
    z-index: 4;
  }

  .tick {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.2rem;
    padding-right: 1.15rem;
    transform: translateY(-50%);
    pointer-events: none;
  }

  .units {
    position: absolute;
    top: 0.2rem;
    right: 0.28rem;
    background: var(--chip-bg);
    color: var(--chip-fg);
    padding: 0.02rem 0.32rem;
    border-radius: 3px;
    box-shadow: 0 0 0 1px var(--chip-ring);
    font-size: 0.66rem;
    font-weight: 600;
    letter-spacing: 0.02em;
    pointer-events: none;
  }

  .label {
    background: var(--chip-bg);
    color: var(--muted);
    padding: 0.02rem 0.3rem;
    border-radius: 3px;
    box-shadow: 0 0 0 1px var(--chip-ring);
    font-size: 0.68rem;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .dash {
    flex: none;
    width: 0.45rem;
    height: 1px;
    background: var(--chip-ring);
  }

  .reset {
    position: absolute;
    bottom: 0.35rem;
    right: 0.28rem;
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
    border: 1px solid color-mix(in oklab, var(--accent) 45%, var(--chrome-strong));
    border-radius: var(--radius-sm);
    background: var(--chip-bg);
    color: var(--accent-strong);
    padding: 0.12rem 0.34rem;
    font-size: 0.66rem;
    pointer-events: auto;
    cursor: pointer;
    box-shadow: var(--shadow-sm);
  }

  .reset :global(svg) {
    font-size: 0.72rem;
  }

  .reset:hover {
    background: var(--accent-tint);
  }
</style>
