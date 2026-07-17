<script lang="ts">
  import IconPencil from '~icons/lucide/pencil';

  interface Props {
    name: string;
    onRename: (next: string) => void;
    /** Rendered class for the display span, so callers can match surrounding typography. */
    class?: string;
    /** aria-label for the edit affordance, e.g. "Rename project" / "Rename recording". */
    label: string;
    testId?: string;
  }

  let { name, onRename, class: className = '', label, testId = 'inline-rename' }: Props = $props();

  let editing = $state(false);
  let draft = $state('');
  let inputEl = $state<HTMLInputElement | null>(null);

  function startEdit() {
    draft = name;
    editing = true;
  }

  function commit() {
    if (!editing) return;
    const next = draft.trim();
    editing = false;
    if (next && next !== name) onRename(next);
  }

  function cancel() {
    editing = false;
  }

  function handleDisplayKeydown(event: KeyboardEvent) {
    if (editing || (event.key !== 'F2' && event.key !== 'Enter' && event.key !== ' ')) return;
    event.preventDefault();
    startEdit();
  }

  function handleInputKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      commit();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      cancel();
    }
  }

  $effect(() => {
    if (editing && inputEl) {
      inputEl.focus();
      inputEl.select();
    }
  });
</script>

<span class="inline-rename" data-testid={testId}>
  {#if editing}
    <input
      bind:this={inputEl}
      bind:value={draft}
      class={className}
      type="text"
      size={Math.max(draft.length + 1, 4)}
      aria-label={label}
      data-testid="{testId}-input"
      onkeydown={handleInputKeydown}
      onblur={commit}
    />
  {:else}
    <span
      class="display {className}"
      data-testid="{testId}-name"
      role="button"
      tabindex="0"
      aria-label={`${label}: ${name}`}
      ondblclick={startEdit}
      onkeydown={handleDisplayKeydown}
    >
      {name}
    </span>
    <button
      type="button"
      class="edit"
      aria-label={label}
      title={label}
      data-testid="{testId}-edit"
      onclick={startEdit}
    >
      <IconPencil aria-hidden="true" />
    </button>
  {/if}
</span>

<style>
  .inline-rename {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    max-width: 100%;
  }

  .display {
    min-width: 0;
    border-radius: var(--radius-sm);
  }

  .display:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .edit {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    padding: 0.15rem;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted);
    opacity: 0;
    transition:
      opacity var(--t-fast),
      color var(--t-fast),
      background var(--t-fast),
      border-color var(--t-fast);
  }

  .edit :global(svg) {
    font-size: 0.85em;
  }

  .inline-rename:hover .edit,
  .inline-rename:focus-within .edit {
    opacity: 1;
  }

  .edit:hover {
    color: var(--accent-strong);
    background: var(--accent-tint);
  }

  .edit:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
    opacity: 1;
  }

  input {
    font: inherit;
    color: var(--text);
    background: var(--panel);
    border: 1px solid var(--accent);
    border-radius: var(--radius-sm);
    padding: 0.05rem 0.35rem;
    min-width: 0;
  }

  input:focus {
    outline: none;
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--accent) 22%, transparent);
  }
</style>
