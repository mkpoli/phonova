<script lang="ts">
  import IconSearch from '~icons/lucide/search';
  import IconChevronLeft from '~icons/lucide/chevron-left';
  import IconChevronRight from '~icons/lucide/chevron-right';

  interface Props {
    query: string;
    count: number;
    index: number;
    onQuery: (text: string) => void;
    onNext: () => void;
    onPrev: () => void;
  }

  let { query, count, index, onQuery, onNext, onPrev }: Props = $props();

  function handleKeydown(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === 'Enter') {
      event.preventDefault();
      if (event.shiftKey) onPrev();
      else onNext();
    }
  }
</script>

<div class="search" data-testid="label-search">
  <div class="search-field">
    <IconSearch class="search-icon" aria-hidden="true" />
    <input
      class="search-input"
      data-testid="search-input"
      type="search"
      placeholder="Search labels"
      autocomplete="off"
      autocapitalize="off"
      autocorrect="off"
      spellcheck="false"
      value={query}
      oninput={(event) => onQuery(event.currentTarget.value)}
      onkeydown={handleKeydown}
    />
  </div>
  <span class="count" data-testid="search-count">{count === 0 ? '0' : `${index + 1}/${count}`}</span>
  <button type="button" aria-label="Previous match" disabled={count === 0} onclick={onPrev}>
    <IconChevronLeft aria-hidden="true" />
  </button>
  <button type="button" aria-label="Next match" disabled={count === 0} onclick={onNext}>
    <IconChevronRight aria-hidden="true" />
  </button>
  {#if query.trim() && count === 0}
    <span class="no-hits" data-testid="search-empty">No labels match.</span>
  {/if}
</div>

<style>
  .search {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .search-field {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-sm);
    background: var(--panel-soft);
    padding: 0 0.45rem;
    transition:
      border-color var(--t-fast),
      box-shadow var(--t-fast);
  }

  .search-field:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--accent) 18%, transparent);
  }

  .search-field :global(.search-icon) {
    flex: none;
    font-size: 0.85rem;
    color: var(--muted);
  }

  .search-input {
    min-width: 8.5rem;
    border: none;
    background: transparent;
    color: var(--text);
    padding: 0.2rem 0;
    font-size: 0.8rem;
    outline: none;
  }

  .count {
    min-width: 2.6rem;
    text-align: center;
    font-size: 0.76rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .no-hits {
    font-size: 0.76rem;
    color: var(--muted);
  }

  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-sm);
    background: var(--panel-soft);
    color: var(--text);
    width: 1.7rem;
    height: 1.7rem;
    line-height: 1;
    transition:
      background var(--t-fast),
      border-color var(--t-fast);
  }

  button :global(svg) {
    font-size: 0.9rem;
  }

  button:hover:not(:disabled) {
    background: var(--panel);
    border-color: color-mix(in oklab, var(--accent) 32%, var(--chrome-strong));
  }

  button:disabled {
    opacity: 0.4;
  }
</style>
