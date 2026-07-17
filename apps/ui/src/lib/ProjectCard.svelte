<script lang="ts">
  import IconCopy from '~icons/lucide/copy';
  import IconTrash from '~icons/lucide/trash-2';
  import InlineRename from './InlineRename.svelte';
  import type { ProjectSummary } from './types';

  interface Props {
    project: ProjectSummary;
    onOpen: (id: string) => void;
    onRename: (id: string, name: string) => void;
    onDelete: (id: string) => void;
    onDuplicate: (id: string) => void;
  }

  let { project, onOpen, onRename, onDelete, onDuplicate }: Props = $props();

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
    onOpen(project.id);
  }

  function handleOpenKeydown(event: KeyboardEvent) {
    if (event.target !== event.currentTarget) return;
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onOpen(project.id);
    }
  }
</script>

<div
  class="card"
  data-testid="project-card"
  data-project-id={project.id}
  data-project-name={project.name}
>
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
