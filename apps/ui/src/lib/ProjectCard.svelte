<script lang="ts">
  import type { ProjectSummary } from './types';

  interface Props {
    project: ProjectSummary;
    onOpen: (id: string) => void;
    onRename: (id: string, currentName: string) => void;
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
</script>

<div
  class="card"
  data-testid="project-card"
  data-project-id={project.id}
  data-project-name={project.name}
>
  <button class="open" type="button" data-testid="open-project" onclick={() => onOpen(project.id)}>
    <span class="name">{project.name}</span>
    <span class="meta">
      {project.count}
      {project.count === 1 ? 'recording' : 'recordings'}
      {#if project.hasRecovery}
        <span class="recovery" data-testid="recovery-badge">unsaved work</span>
      {/if}
    </span>
    <span class="saved">{savedLabel(project.savedAt)}</span>
  </button>
  <div class="actions">
    <button type="button" data-testid="rename-project" onclick={() => onRename(project.id, project.name)}>
      Rename
    </button>
    <button type="button" data-testid="duplicate-project" onclick={() => onDuplicate(project.id)}>
      Duplicate
    </button>
    <button type="button" class="danger" data-testid="delete-project" onclick={() => onDelete(project.id)}>
      Delete
    </button>
  </div>
</div>

<style>
  .card {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--chrome-strong);
    border-radius: 10px;
    background: var(--panel);
    overflow: hidden;
  }

  .open {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    align-items: flex-start;
    padding: 1rem;
    border: 0;
    background: transparent;
    color: var(--text);
    text-align: left;
    min-height: 6.5rem;
  }

  .open:hover {
    background: var(--panel-soft);
  }

  .name {
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
    gap: 0.25rem;
    padding: 0.4rem 0.6rem;
    border-top: 1px solid var(--chrome-strong);
    background: var(--panel-soft);
  }

  .actions button {
    border: 1px solid transparent;
    border-radius: 5px;
    background: transparent;
    color: var(--muted);
    padding: 0.2rem 0.45rem;
    font-size: 0.76rem;
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
