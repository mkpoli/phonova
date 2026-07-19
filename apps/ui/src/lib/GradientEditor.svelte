<script lang="ts">
  import IconCheck from '~icons/lucide/check';
  import IconTriangleAlert from '~icons/lucide/triangle-alert';
  import IconTrash from '~icons/lucide/trash-2';
  import {
    hexToRgb,
    rampGradientCss,
    rampIsMonotonic,
    rgbToHex,
    type CustomRamp,
    type GradientStop
  } from './palette';

  interface Props {
    ramp: CustomRamp;
    /** Fires on every edit, so the caller can drive a live spectrogram preview. */
    onChange: (ramp: CustomRamp) => void;
    onSave: (ramp: CustomRamp) => void;
    onCancel: () => void;
    onDelete?: (id: string) => void;
    /** Whether this ramp already exists in the saved list (enables Delete). */
    existing?: boolean;
  }

  let { ramp, onChange, onSave, onCancel, onDelete, existing = false }: Props = $props();

  // Seeded once from the ramp prop; the editor then owns its own state. The
  // caller remounts this component (keyed on ramp id) when opening a different
  // ramp, so the one-time capture is intentional.
  // svelte-ignore state_referenced_locally
  let name = $state(ramp.name);
  // svelte-ignore state_referenced_locally
  let stops = $state<GradientStop[]>(sortStops(ramp.stops));
  let activeIndex = $state(0);
  let barEl = $state<HTMLDivElement | null>(null);

  const gradient = $derived(rampGradientCss(stops));
  const monotonic = $derived(rampIsMonotonic(stops));
  const activeStop = $derived(stops[activeIndex] ?? stops[0]);

  function sortStops(list: GradientStop[]): GradientStop[] {
    return [...list].sort((a, b) => a.pos - b.pos).map((s) => ({ ...s }));
  }

  function emit() {
    onChange({ ...ramp, name: name.trim() || 'Custom ramp', stops: sortStops(stops) });
  }

  // Sample the current ramp color at a position, for a newly inserted stop.
  function colorAt(pos: number): string {
    const s = sortStops(stops);
    let lo = s[0];
    let hi = s[s.length - 1];
    for (let k = 0; k < s.length - 1; k += 1) {
      if (pos >= s[k].pos && pos <= s[k + 1].pos) {
        lo = s[k];
        hi = s[k + 1];
        break;
      }
    }
    const span = hi.pos - lo.pos;
    const f = span > 0 ? (pos - lo.pos) / span : 0;
    const a = hexToRgb(lo.color);
    const b = hexToRgb(hi.color);
    return rgbToHex([a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f]);
  }

  function posFromClientX(clientX: number): number {
    if (!barEl) return 0;
    const rect = barEl.getBoundingClientRect();
    return Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
  }

  function addStopAt(clientX: number) {
    const pos = posFromClientX(clientX);
    const next = [...stops, { pos, color: colorAt(pos) }];
    stops = sortStops(next);
    activeIndex = stops.findIndex((s) => s.pos === pos);
    emit();
  }

  function onBarPointerDown(event: PointerEvent) {
    // A click on the bar itself (not a handle) adds a stop.
    if (event.target !== barEl) return;
    addStopAt(event.clientX);
  }

  // Drag state for a stop handle.
  let dragIndex = $state<number | null>(null);
  let removing = $state(false);

  function onHandlePointerDown(event: PointerEvent, index: number) {
    event.stopPropagation();
    activeIndex = index;
    dragIndex = index;
    removing = false;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }

  function onHandlePointerMove(event: PointerEvent) {
    if (dragIndex === null || !barEl) return;
    const rect = barEl.getBoundingClientRect();
    const pos = posFromClientX(event.clientX);
    const offBar = Math.abs(event.clientY - (rect.top + rect.height / 2)) > rect.height / 2 + 44;
    removing = offBar && stops.length > 2;
    const id = stops[dragIndex];
    if (!id) return;
    const updated = stops.map((s, i) => (i === dragIndex ? { ...s, pos } : s));
    // Track the dragged stop across a re-sort by identity.
    const sorted = [...updated].sort((a, b) => a.pos - b.pos);
    stops = sorted;
    dragIndex = sorted.indexOf(updated[dragIndex]);
    activeIndex = dragIndex;
    emit();
  }

  function onHandlePointerUp(event: PointerEvent) {
    if (dragIndex === null) return;
    (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
    if (removing && stops.length > 2) {
      stops = stops.filter((_, i) => i !== dragIndex);
      activeIndex = Math.min(activeIndex, stops.length - 1);
      emit();
    }
    dragIndex = null;
    removing = false;
  }

  function setActiveColor(color: string) {
    stops = stops.map((s, i) => (i === activeIndex ? { ...s, color } : s));
    emit();
  }

  function deleteActive() {
    if (stops.length <= 2) return;
    stops = stops.filter((_, i) => i !== activeIndex);
    activeIndex = Math.min(activeIndex, stops.length - 1);
    emit();
  }

  function save() {
    onSave({ ...ramp, name: name.trim() || 'Custom ramp', stops: sortStops(stops) });
  }
</script>

<div class="editor" data-testid="gradient-editor" role="group" aria-label="Gradient editor">
  <header class="head">
    <input
      class="name"
      data-testid="ramp-name"
      aria-label="Ramp name"
      bind:value={name}
      oninput={emit}
      placeholder="Ramp name"
    />
    <span
      class="badge"
      class:ok={monotonic}
      data-testid="monotonic-badge"
      data-monotonic={monotonic}
      title="Relative luminance never decreases from floor to ceiling"
    >
      {#if monotonic}
        <IconCheck aria-hidden="true" />monotonic
      {:else}
        <IconTriangleAlert aria-hidden="true" />non-monotonic
      {/if}
    </span>
  </header>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    bind:this={barEl}
    class="bar"
    data-testid="gradient-bar"
    style="background: {gradient}"
    onpointerdown={onBarPointerDown}
    title="Click to add a stop"
  >
    {#each stops as stop, index (index)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="handle"
        class:active={index === activeIndex}
        class:removing={removing && index === dragIndex}
        data-testid="gradient-stop"
        data-index={index}
        style="left: {stop.pos * 100}%; --swatch: {stop.color}"
        onpointerdown={(e) => onHandlePointerDown(e, index)}
        onpointermove={onHandlePointerMove}
        onpointerup={onHandlePointerUp}
      ></div>
    {/each}
  </div>

  <div class="stop-row">
    <label class="swatch-label">
      <span class="swatch" style="background: {activeStop?.color}"></span>
      <input
        type="color"
        class="color-input"
        data-testid="stop-color"
        aria-label="Stop color"
        value={activeStop?.color ?? '#000000'}
        oninput={(e) => setActiveColor(e.currentTarget.value)}
      />
    </label>
    <span class="pos" data-testid="stop-position"
      >{Math.round((activeStop?.pos ?? 0) * 100)}%</span
    >
    <button
      type="button"
      class="stop-delete"
      data-testid="stop-delete"
      onclick={deleteActive}
      disabled={stops.length <= 2}
      aria-label="Delete stop"
    >
      <IconTrash aria-hidden="true" />
    </button>
    <span class="hint">Click the bar to add · drag a stop off to remove</span>
  </div>

  <footer class="actions">
    {#if existing && onDelete}
      <button
        type="button"
        class="ghost danger"
        data-testid="ramp-remove"
        onclick={() => onDelete?.(ramp.id)}
      >
        Delete ramp
      </button>
    {/if}
    <span class="spacer"></span>
    <button type="button" class="ghost" data-testid="ramp-cancel" onclick={onCancel}>Cancel</button>
    <button type="button" class="primary" data-testid="ramp-save" onclick={save}>Save ramp</button>
  </footer>
</div>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    padding: 0.85rem;
    width: min(28rem, calc(100vw - 2rem));
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-xl);
    background: var(--panel);
    color: var(--text);
    box-shadow: var(--shadow-lg);
  }

  .head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .name {
    flex: 1;
    min-width: 0;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-sm);
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.35rem 0.5rem;
    font: inherit;
    font-weight: 600;
  }

  .name:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    flex: none;
    padding: 0.2rem 0.5rem;
    border-radius: 999px;
    font-size: 0.72rem;
    font-weight: 600;
    background: color-mix(in oklab, var(--warn) 18%, transparent);
    color: var(--warn);
    border: 1px solid color-mix(in oklab, var(--warn) 40%, transparent);
  }

  .badge.ok {
    background: var(--accent-tint);
    color: var(--accent-strong);
    border-color: color-mix(in oklab, var(--accent) 35%, transparent);
  }

  .badge :global(svg) {
    font-size: 0.85rem;
  }

  .bar {
    position: relative;
    height: 2.4rem;
    border-radius: var(--radius-md);
    border: 1px solid var(--chrome-strong);
    cursor: copy;
    touch-action: none;
  }

  .handle {
    position: absolute;
    top: 50%;
    width: 1rem;
    height: 1rem;
    transform: translate(-50%, -50%);
    border-radius: 999px;
    background: var(--swatch);
    border: 2px solid #fff;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.55);
    cursor: grab;
    touch-action: none;
  }

  .handle.active {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .handle.removing {
    opacity: 0.35;
  }

  .stop-row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .swatch-label {
    position: relative;
    display: inline-flex;
    width: 1.8rem;
    height: 1.8rem;
  }

  .swatch {
    width: 100%;
    height: 100%;
    border-radius: var(--radius-sm);
    border: 1px solid var(--chrome-strong);
  }

  .color-input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
  }

  .pos {
    font-variant-numeric: tabular-nums;
    font-size: 0.8rem;
    color: var(--muted);
    min-width: 2.5rem;
  }

  .stop-delete {
    display: inline-grid;
    place-items: center;
    width: 1.8rem;
    height: 1.8rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-sm);
    background: var(--panel-soft);
    color: var(--muted);
    cursor: pointer;
  }

  .stop-delete:hover:not(:disabled) {
    color: var(--warn);
    border-color: color-mix(in oklab, var(--warn) 40%, var(--chrome-strong));
  }

  .stop-delete:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .hint {
    font-size: 0.72rem;
    color: var(--muted);
    margin-left: auto;
    text-align: right;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .spacer {
    flex: 1;
  }

  .actions button {
    border-radius: var(--radius-md);
    padding: 0.4rem 0.85rem;
    font: inherit;
    cursor: pointer;
    border: 1px solid var(--chrome-strong);
    background: var(--panel-soft);
    color: var(--text);
  }

  .ghost:hover {
    background: var(--panel);
  }

  .ghost.danger:hover {
    color: var(--warn);
    border-color: color-mix(in oklab, var(--warn) 40%, var(--chrome-strong));
  }

  .primary {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--on-accent);
  }

  .primary:hover {
    background: var(--accent-strong);
    border-color: var(--accent-strong);
  }
</style>
