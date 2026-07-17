<script lang="ts">
  import type { ViewportState } from './types';

  interface Props {
    viewport: ViewportState;
  }

  let { viewport }: Props = $props();

  let rulerEl = $state<HTMLDivElement | null>(null);
  let rulerWidth = $state(1);

  $effect(() => {
    if (!rulerEl) return;
    const observer = new ResizeObserver(() => {
      rulerWidth = rulerEl?.clientWidth ?? 1;
    });
    observer.observe(rulerEl);
    rulerWidth = rulerEl.clientWidth || 1;
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

  const majorStep = $derived(niceStep(viewport.t1 - viewport.t0, rulerWidth, 80));
  const minorStep = $derived(majorStep / 5);
  const decimals = $derived(tickDecimals(majorStep));

  const majorTicks = $derived.by(() => {
    if (viewport.t1 - viewport.t0 <= 0) return [];
    const ticks: number[] = [];
    for (let t = Math.ceil(viewport.t0 / majorStep) * majorStep; t <= viewport.t1; t += majorStep) {
      ticks.push(t);
    }
    return ticks;
  });

  const minorTicks = $derived.by(() => {
    if (viewport.t1 - viewport.t0 <= 0) return [];
    const eps = majorStep * 1e-6;
    const ticks: number[] = [];
    for (let t = Math.ceil(viewport.t0 / minorStep) * minorStep; t <= viewport.t1; t += minorStep) {
      const k = t / majorStep;
      if (Math.abs(k - Math.round(k)) * majorStep < eps) continue;
      ticks.push(t);
    }
    return ticks;
  });

  function pct(time: number): number {
    return ((time - viewport.t0) / (viewport.t1 - viewport.t0)) * 100;
  }
</script>

<div class="ruler" data-testid="time-ruler" bind:this={rulerEl}>
  {#each minorTicks as t (t)}
    <div class="minor" style="left: {pct(t)}%"></div>
  {/each}
  {#each majorTicks as t (t)}
    <div class="major" style="left: {pct(t)}%">
      <div class="line"></div>
      <div class="label">{t.toFixed(decimals)}</div>
    </div>
  {/each}
  <span class="unit">s</span>
</div>

<style>
  .ruler {
    position: relative;
    height: 1.5rem;
    overflow: hidden;
    background: var(--panel);
    border-bottom: 1px solid var(--chrome-strong);
  }

  .minor {
    position: absolute;
    top: 0;
    width: 1px;
    height: 4px;
    background: var(--chrome-strong);
  }

  .major {
    position: absolute;
    top: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    transform: translateX(-50%);
  }

  .major .line {
    width: 1px;
    height: 8px;
    background: var(--chrome-strong);
  }

  .major .label {
    font-size: 0.68rem;
    line-height: 1.1;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .unit {
    position: absolute;
    top: 0;
    right: 0.25rem;
    font-size: 0.62rem;
    color: var(--muted);
  }
</style>
