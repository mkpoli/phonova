<script lang="ts">
  import type { ViewportState } from './types';

  interface Props {
    viewport: ViewportState;
  }

  let { viewport }: Props = $props();

  let rulerEl = $state<HTMLDivElement | null>(null);
  let rulerHeight = $state(1);

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
</script>

<div class="ruler" data-testid="frequency-ruler" aria-hidden="true" bind:this={rulerEl}>
  {#each majorTicks as f (f)}
    <div class="tick" style="top: {pct(f)}%">
      <span class="label">{f.toFixed(decimals)}</span>
      <span class="dash"></span>
    </div>
  {/each}
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
    pointer-events: none;
    z-index: 3;
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
</style>
