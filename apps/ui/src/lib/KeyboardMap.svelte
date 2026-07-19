<script lang="ts">
  export interface KeyVisualState {
    code: string;
    label: string;
    width: number;
    isModifier: boolean;
    assigned: boolean;
    conflicted: boolean;
    selected: boolean;
    title: string;
  }

  interface Props {
    rows: KeyVisualState[][];
    selectedCode: string | null;
    onSelect: (code: string) => void;
  }

  let { rows, selectedCode, onSelect }: Props = $props();
</script>

<div class="scroller">
  <div class="board" data-testid="keyboard-map">
    {#each rows as row, rowIndex (rowIndex)}
      <div class="row">
        {#each row as key (key.code)}
          <button
            type="button"
            class="key"
            class:modifier={key.isModifier}
            class:assigned={key.assigned}
            class:conflicted={key.conflicted}
            class:selected={key.selected || key.code === selectedCode}
            style:--w={key.width}
            disabled={key.isModifier}
            title={key.title}
            aria-label={key.title}
            data-testid="key"
            data-code={key.code}
            onclick={() => onSelect(key.code)}
          >
            {key.label}
          </button>
        {/each}
      </div>
    {/each}
  </div>
</div>

<style>
  .scroller {
    overflow-x: auto;
    padding: 0.25rem;
  }

  .board {
    --key-size: 2.6rem;
    --key-gap: 0.28rem;

    display: flex;
    flex-direction: column;
    gap: var(--key-gap);
    width: max-content;
  }

  .row {
    display: flex;
    gap: var(--key-gap);
  }

  .key {
    width: calc(var(--w) * (var(--key-size) + var(--key-gap)) - var(--key-gap));
    height: var(--key-size);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 0.2rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-sm);
    background: var(--panel-soft);
    color: var(--text);
    font-size: 0.72rem;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    cursor: pointer;
    transition:
      background var(--t-fast),
      border-color var(--t-fast),
      box-shadow var(--t-fast);
  }

  .key:hover:not(:disabled) {
    background: var(--panel);
  }

  .key:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .key.modifier {
    background: var(--chrome);
    color: var(--muted);
    cursor: default;
    opacity: 0.7;
  }

  .key.assigned {
    background: var(--accent-tint);
    border-color: var(--accent);
    color: var(--accent-strong);
    font-weight: 600;
  }

  .key.assigned:hover:not(:disabled) {
    background: var(--accent-tint);
    border-color: var(--accent-strong);
  }

  .key.conflicted {
    border-color: var(--danger);
    box-shadow: inset 0 0 0 1px var(--danger);
    color: var(--danger);
  }

  .key.selected {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
</style>
