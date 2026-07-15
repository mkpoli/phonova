<script lang="ts">
  import SearchBar from './SearchBar.svelte';
  import TierLane from './TierLane.svelte';
  import type {
    AnnotationClientLike,
    IntervalData,
    LabelHit,
    PointData,
    TierInfo,
    ViewportState
  } from './types';

  interface EditingState {
    tierId: bigint;
    kind: 'interval' | 'point';
    targetId: bigint;
    value: string;
  }

  interface Props {
    client: AnnotationClientLike | null;
    audioId: bigint | null;
    annotationId: bigint | null;
    audioDuration: number;
    sampleRate: number;
    viewport: ViewportState;
    cursorTime: number;
    onSeek?: (time: number) => void;
    onAnnotationChange?: (id: bigint) => void;
  }

  let {
    client,
    audioId,
    annotationId,
    audioDuration,
    sampleRate,
    viewport,
    cursorTime,
    onSeek,
    onAnnotationChange
  }: Props = $props();

  let paneEl = $state<HTMLElement | null>(null);
  let rowsEl = $state<HTMLDivElement | null>(null);
  let rowsWidth = $state(1);
  let fileInput = $state<HTMLInputElement | null>(null);

  let tiers = $state<TierInfo[]>([]);
  let intervalsByTier = $state<Map<bigint, IntervalData[]>>(new Map());
  let pointsByTier = $state<Map<bigint, PointData[]>>(new Map());
  let activeTierId = $state<bigint | null>(null);
  let activeIndex = $state(0);
  let editing = $state<EditingState | null>(null);
  let undoDepth = $state(0);
  let redoDepth = $state(0);
  let stateHash = $state<bigint>(0n);
  let status = $state('');

  let query = $state('');
  let hits = $state<LabelHit[]>([]);
  let hitIndex = $state(0);

  let loadToken = 0;

  const activeTier = $derived(tiers.find((t) => t.id === activeTierId) ?? null);
  const activeIntervals = $derived(
    activeTier && activeTier.kind === 'interval' ? (intervalsByTier.get(activeTier.id) ?? []) : []
  );
  const activePoints = $derived(
    activeTier && activeTier.kind === 'point' ? (pointsByTier.get(activeTier.id) ?? []) : []
  );
  const activeCount = $derived(activeTier?.kind === 'point' ? activePoints.length : activeIntervals.length);

  // Every interior boundary time and every point time in the document — the
  // magnetic targets a dragged boundary snaps to across tiers.
  const snapTimes = $derived.by(() => {
    const times: number[] = [];
    for (const tier of tiers) {
      if (tier.kind === 'interval') {
        const ivs = intervalsByTier.get(tier.id) ?? [];
        for (let i = 0; i < ivs.length - 1; i += 1) times.push(ivs[i].xmax);
      } else {
        for (const point of pointsByTier.get(tier.id) ?? []) times.push(point.time);
      }
    }
    return times;
  });

  $effect(() => {
    // Refetch whenever the document identity changes.
    annotationId;
    void refresh();
  });

  $effect(() => {
    if (!rowsEl) return;
    const observer = new ResizeObserver(() => {
      rowsWidth = rowsEl?.clientWidth ?? 1;
    });
    observer.observe(rowsEl);
    rowsWidth = rowsEl.clientWidth || 1;
    return () => observer.disconnect();
  });

  async function refresh() {
    const ann = annotationId;
    const active = client;
    if (!active || ann === null) {
      tiers = [];
      intervalsByTier = new Map();
      pointsByTier = new Map();
      undoDepth = 0;
      redoDepth = 0;
      return;
    }
    const token = ++loadToken;
    let list: TierInfo[] = [];
    const intervals = new Map<bigint, IntervalData[]>();
    const points = new Map<bigint, PointData[]>();
    try {
      list = await active.annotationTiers(ann);
      if (token !== loadToken) return;
      for (const tier of list) {
        if (tier.kind === 'interval') {
          intervals.set(tier.id, await active.intervalsInRange(ann, tier.id, -1, 1e12));
        } else {
          points.set(tier.id, await active.pointsInRange(ann, tier.id, -1, 1e12));
        }
        if (token !== loadToken) return;
      }
    } catch {
      // The document can vanish mid-read when undo detaches it; render empty.
      if (token !== loadToken) return;
      list = [];
      intervals.clear();
      points.clear();
    }
    tiers = list;
    intervalsByTier = intervals;
    pointsByTier = points;
    if (activeTierId === null || !list.some((t) => t.id === activeTierId)) {
      activeTierId = list[0]?.id ?? null;
      activeIndex = 0;
    }
    clampActiveIndex();
    const [u, r, h] = await Promise.all([active.undoDepth(), active.redoDepth(), active.stateHash()]);
    if (token !== loadToken) return;
    undoDepth = u;
    redoDepth = r;
    stateHash = h;
    if (query) hits = (await active.searchLabels(query, false)).filter((hit) => hit.annotation === ann);
  }

  function clampActiveIndex() {
    const count = activeTier?.kind === 'point'
      ? (pointsByTier.get(activeTier.id)?.length ?? 0)
      : activeTier
        ? (intervalsByTier.get(activeTier.id)?.length ?? 0)
        : 0;
    if (activeIndex > count - 1) activeIndex = Math.max(0, count - 1);
    if (activeIndex < 0) activeIndex = 0;
  }

  function focusPane() {
    paneEl?.focus();
  }

  function itemTime(index: number): number | null {
    if (!activeTier) return null;
    if (activeTier.kind === 'interval') return activeIntervals[index]?.xmin ?? null;
    return activePoints[index]?.time ?? null;
  }

  function selectIndex(index: number) {
    const count = activeCount;
    if (count === 0) return;
    activeIndex = Math.min(count - 1, Math.max(0, index));
    const time = itemTime(activeIndex);
    if (time !== null) onSeek?.(time);
  }

  function activateTier(tierId: bigint, index = 0) {
    activeTierId = tierId;
    activeIndex = index;
    focusPane();
  }

  function focusTierByDigit(digit: number) {
    const tier = tiers[digit - 1];
    if (!tier) return;
    activeTierId = tier.id;
    activeIndex = 0;
    const time = itemTime(0);
    if (time !== null) onSeek?.(time);
  }

  function openEditor(index: number, initial?: string) {
    if (!activeTier) return;
    const items = activeTier.kind === 'interval' ? activeIntervals : activePoints;
    const item = items[index];
    if (!item) return;
    activeIndex = index;
    const current = activeTier.kind === 'interval'
      ? (item as IntervalData).label
      : (item as PointData).label;
    editing = {
      tierId: activeTier.id,
      kind: activeTier.kind,
      targetId: item.id,
      value: initial ?? current
    };
  }

  async function commitEdit() {
    const edit = editing;
    if (!edit || !client || annotationId === null) {
      editing = null;
      return;
    }
    editing = null;
    try {
      if (edit.kind === 'interval') {
        await client.setIntervalLabel(annotationId, edit.tierId, edit.targetId, edit.value);
      } else {
        await client.setPointLabel(annotationId, edit.tierId, edit.targetId, edit.value);
      }
      status = '';
    } catch (error) {
      status = error instanceof Error ? error.message : String(error);
    }
    await refresh();
    focusPane();
  }

  function cancelEdit() {
    editing = null;
    focusPane();
  }

  async function splitAtCursor() {
    if (!client || annotationId === null || activeTier?.kind !== 'interval') return;
    try {
      await client.insertBoundary(annotationId, activeTier.id, cursorTime);
      status = '';
      await refresh();
      selectByTime(cursorTime);
    } catch (error) {
      status = error instanceof Error ? error.message : String(error);
    }
  }

  async function mergeActive() {
    if (!client || annotationId === null || activeTier?.kind !== 'interval') return;
    const ivs = activeIntervals;
    if (ivs.length < 2) return;
    const interval = ivs[activeIndex];
    let boundary: bigint;
    if (activeIndex < ivs.length - 1) boundary = interval.endBoundary;
    else boundary = interval.startBoundary;
    try {
      await client.removeBoundary(annotationId, boundary);
      status = '';
      await refresh();
      clampActiveIndex();
    } catch (error) {
      status = error instanceof Error ? error.message : String(error);
    }
  }

  function activeBoundaryId(): bigint | null {
    const ivs = activeIntervals;
    if (!ivs.length) return null;
    const interval = ivs[activeIndex];
    if (activeIndex > 0) return interval.startBoundary;
    if (ivs.length > 1) return interval.endBoundary;
    return null;
  }

  function boundaryTime(boundaryId: bigint): number | null {
    for (const interval of activeIntervals) {
      if (interval.startBoundary === boundaryId) return interval.xmin;
      if (interval.endBoundary === boundaryId) return interval.xmax;
    }
    return null;
  }

  async function nudgeBoundary(direction: number, oneFrame: boolean) {
    if (!client || annotationId === null || activeTier?.kind !== 'interval') return;
    const boundary = activeBoundaryId();
    if (boundary === null) return;
    const from = boundaryTime(boundary);
    if (from === null) return;
    const pixelSeconds = (viewport.t1 - viewport.t0) / Math.max(1, rowsWidth);
    const step = oneFrame && sampleRate > 0 ? 1 / sampleRate : pixelSeconds;
    try {
      await client.moveBoundary(annotationId, boundary, from + direction * step, true);
      status = '';
      await refresh();
    } catch (error) {
      status = error instanceof Error ? error.message : String(error);
    }
  }

  async function moveBoundaryTo(boundaryId: bigint, toTime: number) {
    if (!client || annotationId === null) return;
    try {
      await client.moveBoundary(annotationId, boundaryId, toTime, true);
      status = '';
      await refresh();
    } catch (error) {
      status = error instanceof Error ? error.message : String(error);
    }
  }

  function selectByTime(time: number) {
    if (activeTier?.kind !== 'interval') return;
    const index = activeIntervals.findIndex((iv) => time >= iv.xmin && time < iv.xmax);
    if (index >= 0) activeIndex = index;
  }

  async function addTier(kind: 'interval' | 'point') {
    if (!client || annotationId === null) return;
    const name = `${kind} ${tiers.filter((t) => t.kind === kind).length + 1}`;
    try {
      const id = kind === 'interval'
        ? await client.addIntervalTier(annotationId, name)
        : await client.addPointTier(annotationId, name);
      await refresh();
      activateTier(id, 0);
    } catch (error) {
      status = error instanceof Error ? error.message : String(error);
    }
  }

  async function removeTier(tierId: bigint) {
    if (!client || annotationId === null) return;
    try {
      await client.removeTier(annotationId, tierId);
      if (activeTierId === tierId) activeTierId = null;
      await refresh();
    } catch (error) {
      status = error instanceof Error ? error.message : String(error);
    }
  }

  async function undo() {
    if (!client) return;
    editing = null;
    await client.undo();
    await refresh();
  }

  async function redo() {
    if (!client) return;
    editing = null;
    await client.redo();
    await refresh();
  }

  async function runSearch(text: string) {
    query = text;
    if (!client || annotationId === null || !text) {
      hits = [];
      hitIndex = 0;
      return;
    }
    const found = await client.searchLabels(text, false);
    hits = found.filter((hit) => hit.annotation === annotationId);
    hitIndex = 0;
    goToHit();
  }

  function goToHit() {
    const hit = hits[hitIndex];
    if (!hit) return;
    const tier = tiers.find((t) => t.id === hit.tier);
    if (!tier) return;
    activeTierId = tier.id;
    if (tier.kind === 'interval') {
      const ivs = intervalsByTier.get(tier.id) ?? [];
      const index = ivs.findIndex((iv) => iv.id === hit.target);
      if (index >= 0) {
        activeIndex = index;
        onSeek?.(ivs[index].xmin);
      }
    } else {
      const pts = pointsByTier.get(tier.id) ?? [];
      const index = pts.findIndex((pt) => pt.id === hit.target);
      if (index >= 0) {
        activeIndex = index;
        onSeek?.(pts[index].time);
      }
    }
  }

  function nextHit() {
    if (hits.length === 0) return;
    hitIndex = (hitIndex + 1) % hits.length;
    goToHit();
  }

  function prevHit() {
    if (hits.length === 0) return;
    hitIndex = (hitIndex - 1 + hits.length) % hits.length;
    goToHit();
  }

  async function exportTextGrid() {
    if (!client || annotationId === null) return;
    const bytes = await client.exportTextGrid(annotationId);
    const blob = new Blob([bytes as BlobPart], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = 'annotation.TextGrid';
    anchor.click();
    URL.revokeObjectURL(url);
  }

  async function importTextGridFile(file: File) {
    if (!client || audioId === null) return;
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const newId = await client.importTextGrid(audioId, bytes);
      activeTierId = null;
      onAnnotationChange?.(newId);
      status = '';
    } catch (error) {
      status = error instanceof Error ? error.message : String(error);
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (editing) return;
    if (annotationId === null) return;
    // Keys typed into toolbar controls (buttons, the search field) keep their
    // native behavior; the annotation loop reads keys only from the pane body.
    const target = event.target;
    if (
      target instanceof HTMLButtonElement ||
      target instanceof HTMLInputElement ||
      target instanceof HTMLSelectElement
    ) {
      return;
    }
    const { key } = event;

    if (/^[1-9]$/.test(key)) {
      event.preventDefault();
      event.stopPropagation();
      focusTierByDigit(Number(key));
      return;
    }
    if (key === 'Tab') {
      event.preventDefault();
      event.stopPropagation();
      selectIndex(activeIndex + (event.shiftKey ? -1 : 1));
      return;
    }
    if (key === 'Enter') {
      event.preventDefault();
      event.stopPropagation();
      openEditor(activeIndex);
      return;
    }
    if (key === 'ArrowLeft' || key === 'ArrowRight') {
      event.preventDefault();
      event.stopPropagation();
      void nudgeBoundary(key === 'ArrowLeft' ? -1 : 1, event.altKey);
      return;
    }
    if (key === 's' || key === 'S') {
      event.preventDefault();
      event.stopPropagation();
      void splitAtCursor();
      return;
    }
    if (key === 'm' || key === 'M') {
      event.preventDefault();
      event.stopPropagation();
      void mergeActive();
      return;
    }
    // Type-to-edit: a printable character opens the label editor seeded with it.
    if (key.length === 1 && key !== ' ' && !event.ctrlKey && !event.metaKey && !event.altKey) {
      event.preventDefault();
      event.stopPropagation();
      openEditor(activeIndex, key);
    }
  }

  function handleGlobalKeydown(event: KeyboardEvent) {
    const target = event.target;
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return;
    if (!(event.ctrlKey || event.metaKey)) return;
    const lower = event.key.toLowerCase();
    if (lower === 'z') {
      event.preventDefault();
      if (event.shiftKey) void redo();
      else void undo();
    } else if (lower === 'y') {
      event.preventDefault();
      void redo();
    }
  }
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<!-- A focusable editing surface: the annotation loop is keyboard-driven, so the
     pane takes focus and key events directly (the a11y rules below assume
     content, not an editor). -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="tier-pane"
  data-testid="tier-pane"
  data-tier-count={tiers.length}
  data-undo-depth={undoDepth}
  data-redo-depth={redoDepth}
  data-state-hash={stateHash.toString()}
  data-active-tier={activeTier?.name ?? ''}
  data-active-index={activeIndex}
  role="application"
  aria-label="Annotation tiers"
  tabindex="0"
  bind:this={paneEl}
  onkeydown={handleKeydown}
>
  <div class="anno-toolbar" role="toolbar" aria-label="Annotation actions" tabindex="-1" onpointerdown={(event) => event.stopPropagation()}>
    <button type="button" data-testid="add-interval-tier" disabled={annotationId === null} onclick={() => addTier('interval')}>+ Interval tier</button>
    <button type="button" data-testid="add-point-tier" disabled={annotationId === null} onclick={() => addTier('point')}>+ Point tier</button>
    <div class="spacer"></div>
    <SearchBar query={query} count={hits.length} index={hitIndex} onQuery={runSearch} onNext={nextHit} onPrev={prevHit} />
    <button type="button" data-testid="import-textgrid" disabled={audioId === null} onclick={() => fileInput?.click()}>Import TextGrid</button>
    <button type="button" data-testid="export-textgrid" disabled={annotationId === null} onclick={exportTextGrid}>Export TextGrid</button>
    <input
      bind:this={fileInput}
      class="hidden-input"
      data-testid="textgrid-input"
      type="file"
      accept=".TextGrid,.textgrid,text/plain"
      onchange={(event) => {
        const file = event.currentTarget.files?.item(0);
        if (file) void importTextGridFile(file);
        event.currentTarget.value = '';
      }}
    />
  </div>

  <div class="tier-rows" bind:this={rowsEl}>
    {#if tiers.length === 0}
      <div class="empty" data-testid="tier-empty">Add a tier to start annotating.</div>
    {/if}
    {#each tiers as tier, tierIndex (tier.id)}
      <div class="tier-row">
        <TierLane
          tier={tier}
          intervals={intervalsByTier.get(tier.id) ?? []}
          points={pointsByTier.get(tier.id) ?? []}
          viewport={viewport}
          active={tier.id === activeTierId}
          activeIndex={activeIndex}
          editing={editing && editing.tierId === tier.id ? { targetId: editing.targetId, value: editing.value } : null}
          snapTimes={snapTimes}
          cursorTime={cursorTime}
          onActivate={(index) => activateTier(tier.id, index)}
          onEditRequest={(index) => { activateTier(tier.id, index); openEditor(index); }}
          onMoveBoundary={moveBoundaryTo}
          onEditInput={(value) => { if (editing) editing = { ...editing, value }; }}
          onEditCommit={commitEdit}
          onEditCancel={cancelEdit}
        />
        <div class="tier-chip">
          <button type="button" class="tier-name" data-testid="tier-name" onclick={() => activateTier(tier.id)}>
            <span class="tier-digit">{tierIndex + 1}</span>{tier.name}
          </button>
          <button type="button" class="tier-remove" aria-label={`Remove ${tier.name}`} data-testid="remove-tier" onclick={() => removeTier(tier.id)}>×</button>
        </div>
      </div>
    {/each}
  </div>

  {#if status}
    <div class="status" role="status" data-testid="tier-status">{status}</div>
  {/if}
</div>

<style>
  .tier-pane {
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--panel);
    outline: none;
  }

  .tier-pane:focus-visible {
    box-shadow: inset 0 0 0 2px color-mix(in oklab, var(--accent), transparent 55%);
  }

  .anno-toolbar {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.35rem 0.6rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--panel-soft);
    flex-wrap: wrap;
  }

  .anno-toolbar .spacer {
    flex: 1 1 auto;
  }

  .anno-toolbar button {
    border: 1px solid var(--chrome-strong);
    border-radius: 5px;
    background: var(--panel);
    color: var(--text);
    padding: 0.15rem 0.5rem;
    font-size: 0.8rem;
  }

  .anno-toolbar button:disabled {
    opacity: 0.45;
  }

  .tier-rows {
    position: relative;
    overflow-y: auto;
    min-height: 2.5rem;
  }

  .tier-row {
    position: relative;
  }

  .tier-chip {
    position: absolute;
    top: 0.2rem;
    left: 0.3rem;
    display: flex;
    align-items: stretch;
    gap: 1px;
    z-index: 4;
    font-size: 0.72rem;
    border-radius: 4px;
    overflow: hidden;
    box-shadow: 0 0 0 1px var(--chip-ring);
  }

  .tier-name {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    border: none;
    background: var(--chip-bg);
    color: var(--chip-fg);
    padding: 0.1rem 0.4rem;
    font-size: 0.72rem;
  }

  .tier-digit {
    display: inline-grid;
    place-items: center;
    width: 1rem;
    height: 1rem;
    border-radius: 3px;
    background: color-mix(in oklab, var(--accent), transparent 55%);
    color: var(--chip-fg);
    font-variant-numeric: tabular-nums;
  }

  .tier-remove {
    border: none;
    background: var(--chip-bg);
    color: var(--muted);
    padding: 0 0.4rem;
    font-size: 0.85rem;
    line-height: 1;
  }

  .tier-remove:hover {
    color: var(--warn);
  }

  .empty {
    padding: 0.75rem 0.6rem;
    color: var(--muted);
    font-size: 0.85rem;
  }

  .hidden-input {
    display: none;
  }

  .status {
    padding: 0.25rem 0.6rem;
    color: var(--warn);
    font-size: 0.78rem;
    border-top: 1px solid var(--chrome-strong);
  }
</style>
