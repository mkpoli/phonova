<script lang="ts">
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
  <span class="count" data-testid="search-count">{count === 0 ? '0' : `${index + 1}/${count}`}</span>
  <button type="button" aria-label="Previous match" disabled={count === 0} onclick={onPrev}>‹</button>
  <button type="button" aria-label="Next match" disabled={count === 0} onclick={onNext}>›</button>
</div>

<style>
  .search {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .search-input {
    min-width: 9rem;
    border: 1px solid var(--chrome-strong);
    border-radius: 5px;
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.15rem 0.45rem;
    font-size: 0.8rem;
  }

  .count {
    min-width: 2.6rem;
    text-align: center;
    font-size: 0.76rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  button {
    border: 1px solid var(--chrome-strong);
    border-radius: 5px;
    background: var(--panel-soft);
    color: var(--text);
    width: 1.6rem;
    height: 1.6rem;
    line-height: 1;
    font-size: 1rem;
  }

  button:disabled {
    opacity: 0.4;
  }
</style>
