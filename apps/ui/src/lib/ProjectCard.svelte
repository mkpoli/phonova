<script lang="ts">
  import IconCopy from '~icons/lucide/copy';
  import IconTrash from '~icons/lucide/trash-2';
  import IconPin from '~icons/lucide/pin';
  import IconGripVertical from '~icons/lucide/grip-vertical';
  import InlineRename from './InlineRename.svelte';
  import type { ProjectSummary } from './types';

  interface Props {
    project: ProjectSummary;
    /** Whether this card is pinned (fills the pin, moves it to the Pinned section). */
    pinned?: boolean;
    /** Whether this card is part of the current multi-selection. */
    selected?: boolean;
    /** Body activation: a plain click opens, a modified click selects. Handled upstream. */
    onActivate: (id: string, event: MouseEvent | KeyboardEvent) => void;
    onRename: (id: string, name: string) => void;
    onDelete: (id: string) => void;
    onDuplicate: (id: string) => void;
    /** Toggles the pin; absent hides the pin control (desktop shell). */
    onTogglePin?: (id: string) => void;
    /** Begins a pointer drag to move the card between groups; absent hides the handle. */
    onDragStart?: (id: string, event: PointerEvent) => void;
    /** True while this card is the one being dragged. */
    dragging?: boolean;
  }

  let {
    project,
    pinned = false,
    selected = false,
    onActivate,
    onRename,
    onDelete,
    onDuplicate,
    onTogglePin,
    onDragStart,
    dragging = false
  }: Props = $props();

  function savedLabel(ms: number): string {
    const date = new Date(ms);
    if (Number.isNaN(date.getTime())) return '';
    return date.toLocaleString(undefined, {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    });
  }

  function handleOpenClick(event: MouseEvent) {
    if (event.target instanceof Element && event.target.closest('.inline-rename')) return;
    onActivate(project.id, event);
  }

  function handleOpenKeydown(event: KeyboardEvent) {
    if (event.target !== event.currentTarget) return;
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onActivate(project.id, event);
    }
  }
</script>

<div
  class="card"
  class:pinned
  class:selected
  class:dragging
  data-testid="project-card"
  data-project-id={project.id}
  data-project-name={project.name}
  data-selected={selected}
  data-pinned={pinned}
>
  <div class="corner">
    {#if onDragStart}
      <button
        type="button"
        class="grip"
        aria-label="Drag to move between groups"
        title="Drag to move between groups"
        data-testid="card-drag"
        onpointerdown={(event) => onDragStart?.(project.id, event)}
      >
        <IconGripVertical aria-hidden="true" />
      </button>
    {/if}
    {#if onTogglePin}
      <button
        type="button"
        class="pin"
        class:on={pinned}
        aria-label={pinned ? 'Unpin project' : 'Pin project'}
        aria-pressed={pinned}
        title={pinned ? 'Unpin project' : 'Pin to top'}
        data-testid="pin-project"
        onclick={() => onTogglePin?.(project.id)}
      >
        <IconPin aria-hidden="true" />
      </button>
    {/if}
  </div>

  <div
    class="open"
    role="button"
    tabindex="0"
    data-testid="open-project"
    onclick={handleOpenClick}
    onkeydown={handleOpenKeydown}
  >
    <InlineRename
      name={project.name}
      class="card-name"
      label="Rename project"
      testId="rename-project"
      onRename={(next) => onRename(project.id, next)}
    />
    <span class="meta">
      {project.count}
      {project.count === 1 ? 'recording' : 'recordings'}
      {#if project.hasRecovery}
        <span class="recovery" data-testid="recovery-badge">unsaved work</span>
      {/if}
    </span>
    <span class="saved">{savedLabel(project.savedAt)}</span>
  </div>
  <div class="actions">
    <button type="button" data-testid="duplicate-project" onclick={() => onDuplicate(project.id)}>
      <IconCopy aria-hidden="true" /><span>Duplicate</span>
    </button>
    <button type="button" class="danger" data-testid="delete-project" onclick={() => onDelete(project.id)}>
      <IconTrash aria-hidden="true" /><span>Delete</span>
    </button>
  </div>
</div>

<style>
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-xl);
    background: var(--panel);
    overflow: hidden;
    box-shadow: var(--shadow-sm);
    transition:
      box-shadow var(--t),
      border-color var(--t),
      transform var(--t);
  }

  .card:hover {
    box-shadow: var(--shadow-md);
    border-color: color-mix(in oklab, var(--accent) 28%, var(--chrome-strong));
    transform: translateY(-1px);
  }

  /* Selection is the identity accent: a ring plus a tinted surface. */
  .card.selected {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 2px var(--accent);
  }

  .card.dragging {
    opacity: 0.5;
  }

  .corner {
    position: absolute;
    top: 0.4rem;
    right: 0.4rem;
    display: flex;
    align-items: center;
    gap: 0.15rem;
    z-index: 1;
  }

  .grip,
  .pin {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted);
    padding: 0.22rem;
    transition:
      opacity var(--t-fast),
      color var(--t-fast),
      background var(--t-fast),
      border-color var(--t-fast);
  }

  .grip {
    opacity: 0;
    cursor: grab;
    touch-action: none;
  }

  .card:hover .grip,
  .card:focus-within .grip {
    opacity: 1;
  }

  .grip:hover {
    color: var(--text);
    background: var(--panel-soft);
    border-color: var(--chrome-strong);
  }

  .pin {
    opacity: 0;
  }

  .card:hover .pin,
  .card:focus-within .pin,
  .pin.on {
    opacity: 1;
  }

  .pin:hover {
    color: var(--accent-strong);
    background: var(--accent-tint);
  }

  .pin.on {
    color: var(--accent-strong);
  }

  .pin.on :global(svg) {
    fill: currentColor;
  }

  .grip :global(svg),
  .pin :global(svg) {
    font-size: 0.95rem;
  }

  .open {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    align-items: flex-start;
    padding: 1rem;
    color: var(--text);
    text-align: left;
    min-height: 6.5rem;
    cursor: pointer;
  }

  .card.selected .open {
    background: var(--accent-tint);
  }

  .open:hover {
    background: var(--accent-tint);
  }

  .open:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .open :global(.card-name) {
    font-size: 1.02rem;
    font-weight: 600;
    line-height: 1.25;
  }

  .meta {
    font-size: 0.82rem;
    color: var(--muted);
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .recovery {
    padding: 0.05rem 0.4rem;
    border-radius: 999px;
    background: color-mix(in oklab, var(--warn) 22%, transparent);
    color: var(--warn);
    font-size: 0.72rem;
  }

  .saved {
    margin-top: auto;
    font-size: 0.76rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .actions {
    display: flex;
    gap: 0.2rem;
    padding: 0.4rem 0.5rem;
    border-top: 1px solid var(--chrome-strong);
    background: var(--panel-soft);
  }

  .actions button {
    display: inline-flex;
    align-items: center;
    gap: 0.28rem;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted);
    padding: 0.2rem 0.4rem;
    font-size: 0.75rem;
    white-space: nowrap;
    transition:
      background var(--t-fast),
      border-color var(--t-fast),
      color var(--t-fast);
  }

  .actions button :global(svg) {
    font-size: 0.82rem;
    flex: none;
  }

  .actions button:hover {
    color: var(--text);
    border-color: var(--chrome-strong);
    background: var(--panel);
  }

  .actions .danger:hover {
    color: var(--warn);
    border-color: color-mix(in oklab, var(--warn) 45%, transparent);
  }
</style>
