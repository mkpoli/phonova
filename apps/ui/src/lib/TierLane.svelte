<script lang="ts">
  import BoundaryHandle from './BoundaryHandle.svelte';
  import LabelEditor from './LabelEditor.svelte';
  import type { IntervalData, PointData, TierInfo } from './types';

  interface EditingState {
    targetId: bigint;
    value: string;
  }

  interface Props {
    tier: TierInfo;
    intervals: IntervalData[];
    points: PointData[];
    viewport: { t0: number; t1: number };
    active: boolean;
    activeIndex: number;
    editing: EditingState | null;
    snapTimes: number[];
    cursorTime: number;
    onActivate: (index: number) => void;
    onEditRequest: (index: number) => void;
    onMoveBoundary: (boundaryId: bigint, toTime: number) => void;
    onEditInput: (value: string) => void;
    onEditCommit: () => void;
    onEditCancel: () => void;
  }

  let {
    tier,
    intervals,
    points,
    viewport,
    active,
    activeIndex,
    editing,
    snapTimes,
    cursorTime,
    onActivate,
    onEditRequest,
    onMoveBoundary,
    onEditInput,
    onEditCommit,
    onEditCancel
  }: Props = $props();

  let laneEl = $state<HTMLDivElement | null>(null);
  let draggingBoundary = $state<bigint | null>(null);
  let dragTime = $state(0);

  const span = $derived(Math.max(1e-9, viewport.t1 - viewport.t0));

  function pct(time: number) {
    return ((time - viewport.t0) / span) * 100;
  }

  function xToTime(clientX: number) {
    const rect = laneEl?.getBoundingClientRect();
    if (!rect || rect.width === 0) return viewport.t0;
    const ratio = (clientX - rect.left) / rect.width;
    return viewport.t0 + ratio * span;
  }

  // Movable range of an interior boundary: strictly between the far edges of the
  // two intervals it separates, so a committed move never collapses an interval.
  function boundaryRange(boundaryId: bigint): { lo: number; hi: number } | null {
    const i = intervals.findIndex((iv) => iv.endBoundary === boundaryId);
    if (i < 0 || i + 1 >= intervals.length) return null;
    return { lo: intervals[i].xmin, hi: intervals[i + 1].xmax };
  }

  function snap(time: number, range: { lo: number; hi: number }) {
    const rect = laneEl?.getBoundingClientRect();
    const width = rect?.width ?? 1;
    const thresholdT = (6 / Math.max(1, width)) * span;
    let best = time;
    let bestDist = thresholdT;
    for (const candidate of [cursorTime, ...snapTimes]) {
      if (candidate <= range.lo || candidate >= range.hi) continue;
      const dist = Math.abs(candidate - time);
      if (dist < bestDist) {
        best = candidate;
        bestDist = dist;
      }
    }
    return best;
  }

  function clampToRange(time: number, range: { lo: number; hi: number }) {
    const eps = span * 1e-4;
    return Math.min(range.hi - eps, Math.max(range.lo + eps, time));
  }

  function grab(boundaryId: bigint, clientX: number) {
    const range = boundaryRange(boundaryId);
    if (!range) return;
    draggingBoundary = boundaryId;
    dragTime = clampToRange(xToTime(clientX), range);
  }

  function drag(boundaryId: bigint, clientX: number) {
    const range = boundaryRange(boundaryId);
    if (!range) return;
    dragTime = clampToRange(snap(xToTime(clientX), range), range);
  }

  function release(boundaryId: bigint, clientX: number) {
    const range = boundaryRange(boundaryId);
    draggingBoundary = null;
    if (!range) return;
    const to = clampToRange(snap(xToTime(clientX), range), range);
    onMoveBoundary(boundaryId, to);
  }

  function boundaryLeftPct(boundaryId: bigint, fallbackTime: number) {
    if (draggingBoundary === boundaryId) return pct(dragTime);
    return pct(fallbackTime);
  }
</script>

<div class="lane" class:active data-testid="tier-lane" data-tier-name={tier.name} data-tier-kind={tier.kind} bind:this={laneEl}>
  {#if tier.kind === 'interval'}
    {#each intervals as interval, index (interval.id)}
      <div
        class="interval"
        class:selected={active && index === activeIndex}
        data-testid="interval"
        data-label={interval.label}
        data-xmin={interval.xmin.toFixed(9)}
        data-xmax={interval.xmax.toFixed(9)}
        style={`left:${pct(interval.xmin)}%;width:${pct(interval.xmax) - pct(interval.xmin)}%`}
        role="button"
        tabindex="-1"
        onpointerdown={() => onActivate(index)}
        ondblclick={() => onEditRequest(index)}
      >
        {#if editing && editing.targetId === interval.id}
          <LabelEditor value={editing.value} onInput={onEditInput} onCommit={onEditCommit} onCancel={onEditCancel} />
        {:else}
          <span class="label">{interval.label}</span>
        {/if}
      </div>
    {/each}
    {#each intervals.slice(0, -1) as interval (interval.endBoundary)}
      <BoundaryHandle
        leftPct={boundaryLeftPct(interval.endBoundary, interval.xmax)}
        active={active}
        linked={false}
        dragging={draggingBoundary === interval.endBoundary}
        onGrab={(clientX) => grab(interval.endBoundary, clientX)}
        onDrag={(clientX) => drag(interval.endBoundary, clientX)}
        onRelease={(clientX) => release(interval.endBoundary, clientX)}
      />
    {/each}
  {:else}
    {#each points as point, index (point.id)}
      <div
        class="point"
        class:selected={active && index === activeIndex}
        data-testid="point"
        data-label={point.label}
        data-time={point.time.toFixed(6)}
        style={`left:${pct(point.time)}%`}
        role="button"
        tabindex="-1"
        onpointerdown={() => onActivate(index)}
        ondblclick={() => onEditRequest(index)}
      >
        <span class="point-line"></span>
        {#if editing && editing.targetId === point.id}
          <div class="point-editor">
            <LabelEditor value={editing.value} onInput={onEditInput} onCommit={onEditCommit} onCancel={onEditCancel} />
          </div>
        {:else}
          <span class="point-label">{point.label}</span>
        {/if}
      </div>
    {/each}
  {/if}
</div>

<style>
  .lane {
    position: relative;
    /* Tall enough that the bottom-anchored label band clears the tier chip that
       overlays the top-left corner; at 2.5rem the two touched and an early
       label ghosted behind its chip. */
    height: 3rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--panel);
    overflow: hidden;
    user-select: none;
  }

  .lane.active {
    background: color-mix(in oklab, var(--accent), var(--panel) 92%);
  }

  .interval {
    position: absolute;
    top: 0;
    bottom: 0;
    display: flex;
    /* Labels sit in the lower half of the lane; the upper band belongs to the
       tier chip, so an early label is never hidden behind it. */
    align-items: flex-end;
    justify-content: center;
    border-right: 1px solid transparent;
    overflow: hidden;
    transition: background var(--t-fast);
  }

  .interval:hover {
    background: color-mix(in oklab, var(--accent), transparent 92%);
  }

  .interval.selected {
    background: color-mix(in oklab, var(--accent), transparent 84%);
    box-shadow: inset 0 0 0 1px var(--accent);
  }

  .interval.selected:hover {
    background: color-mix(in oklab, var(--accent), transparent 84%);
  }

  .label {
    padding: 0 0.3rem 0.3rem;
    font-family: var(--font-ipa);
    font-size: 0.85rem;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    pointer-events: none;
  }

  .point {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .point-line {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    margin-left: -1px;
    background: var(--accent);
    transition: background var(--t-fast);
  }

  .point:hover .point-line {
    background: var(--accent-strong);
  }

  .point.selected .point-line {
    background: var(--accent-strong);
    width: 3px;
    margin-left: -1.5px;
  }

  .point.selected:hover .point-line {
    background: var(--accent-strong);
  }

  .point-label {
    position: absolute;
    bottom: 0.2rem;
    left: 0.2rem;
    font-family: var(--font-ipa);
    font-size: 0.78rem;
    color: var(--text);
    background: var(--chip-bg);
    padding: 0 0.2rem;
    border-radius: 3px;
    white-space: nowrap;
    pointer-events: none;
  }

  .point-editor {
    position: absolute;
    bottom: 0.15rem;
    left: 0.2rem;
    width: 6rem;
    height: 1.6rem;
  }
</style>
