<script lang="ts">
  import IconSearch from '~icons/lucide/search';
  import {
    COMMAND_GROUP_ORDER,
    commandShortcut,
    searchCommands,
    type Command,
    type CommandGroup,
    type CommandRegistry
  } from './commands.svelte';

  interface Props {
    registry: CommandRegistry | null;
  }

  let { registry }: Props = $props();

  let open = $state(false);
  let query = $state('');
  let activeIndex = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);

  interface Section {
    key: string;
    label: string;
    items: Command[];
  }

  // Actions runnable on the current screen: registered and, where an action
  // gates on state, enabled right now.
  const available = $derived(
    (registry?.commands ?? []).filter((command) => !command.enabled || command.enabled())
  );

  const sections = $derived.by<Section[]>(() => {
    const trimmed = query.trim();
    if (trimmed) {
      const ranked = searchCommands(available, trimmed);
      return ranked.length ? [{ key: 'results', label: 'Results', items: ranked }] : [];
    }
    const out: Section[] = [];
    const recentIds = registry?.recent ?? [];
    const recent = recentIds
      .map((id) => available.find((command) => command.id === id))
      .filter((command): command is Command => command !== undefined);
    if (recent.length) out.push({ key: 'recent', label: 'Recent', items: recent });
    for (const group of COMMAND_GROUP_ORDER) {
      const items = available.filter((command) => command.group === group);
      if (items.length) out.push({ key: group, label: group, items });
    }
    return out;
  });

  const flat = $derived(sections.flatMap((section) => section.items));

  // Keep the highlight inside the current result list as it shrinks or grows.
  $effect(() => {
    if (activeIndex > flat.length - 1) activeIndex = Math.max(0, flat.length - 1);
  });

  $effect(() => {
    if (open && inputEl) inputEl.focus();
  });

  function show() {
    query = '';
    activeIndex = 0;
    open = true;
  }

  function hide() {
    open = false;
  }

  function flatIndex(section: Section, itemIndex: number): number {
    let base = 0;
    for (const current of sections) {
      if (current === section) break;
      base += current.items.length;
    }
    return base + itemIndex;
  }

  async function execute(command: Command | undefined) {
    if (!command || !registry) return;
    hide();
    await registry.run(command.id);
  }

  function onWindowKeydown(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
      event.preventDefault();
      if (open) hide();
      else show();
    }
  }

  function onPaletteKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      hide();
      return;
    }
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      if (flat.length) activeIndex = (activeIndex + 1) % flat.length;
      return;
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      if (flat.length) activeIndex = (activeIndex - 1 + flat.length) % flat.length;
      return;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      void execute(flat[activeIndex]);
    }
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

{#if open}
  <div
    class="palette-backdrop"
    data-testid="command-palette"
    role="presentation"
    onpointerdown={(event) => {
      if (event.target === event.currentTarget) hide();
    }}
  >
    <div class="palette" role="dialog" aria-modal="true" aria-label="Command palette">
      <div class="palette-search">
        <IconSearch class="search-icon" aria-hidden="true" />
        <input
          bind:this={inputEl}
          class="palette-input"
          data-testid="command-palette-input"
          type="text"
          placeholder="Search actions…"
          aria-label="Search actions"
          bind:value={query}
          onkeydown={onPaletteKeydown}
        />
      </div>
      <div class="palette-list" role="listbox" aria-label="Actions">
        {#if flat.length === 0}
          <p class="palette-empty" data-testid="command-palette-empty">No matching action.</p>
        {/if}
        {#each sections as section (section.key)}
          <div class="palette-group" data-testid="command-group" data-group={section.label}>
            {section.label}
          </div>
          {#each section.items as command, itemIndex (command.id)}
            {@const index = flatIndex(section, itemIndex)}
            {@const shortcut = commandShortcut(command)}
            <button
              type="button"
              class="palette-item"
              class:selected={index === activeIndex}
              data-testid="command-item"
              data-command-id={command.id}
              data-selected={index === activeIndex}
              role="option"
              aria-selected={index === activeIndex}
              onpointermove={() => (activeIndex = index)}
              onclick={() => void execute(command)}
            >
              <span class="item-main">
                <span class="item-title">{command.title}</span>
                {#if command.api && command.api.length}
                  <span class="item-api">{command.api.join(' · ')}</span>
                {/if}
              </span>
              {#if shortcut}
                <kbd class="item-shortcut">{shortcut}</kbd>
              {/if}
            </button>
          {/each}
        {/each}
      </div>
    </div>
  </div>
{/if}

<style>
  .palette-backdrop {
    position: fixed;
    inset: 0;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
    background: rgba(15, 23, 42, 0.42);
    backdrop-filter: blur(3px);
    -webkit-backdrop-filter: blur(3px);
    z-index: 40;
  }

  .palette {
    width: min(38rem, calc(100vw - 2rem));
    max-height: 70vh;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-xl);
    background: var(--panel);
    color: var(--text);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
  }

  .palette-search {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0 1rem;
    border-bottom: 1px solid var(--chrome-strong);
  }

  .palette-search :global(.search-icon) {
    flex: none;
    font-size: 1.05rem;
    color: var(--muted);
  }

  .palette-input {
    flex: 1;
    border: none;
    background: transparent;
    color: var(--text);
    padding: 0.9rem 0;
    font-size: 1rem;
    outline: none;
  }

  .palette-list {
    overflow-y: auto;
    padding: 0.35rem 0;
  }

  .palette-empty {
    margin: 0;
    padding: 0.85rem 1rem;
    color: var(--muted);
    font-size: 0.88rem;
  }

  .palette-group {
    padding: 0.5rem 1rem 0.25rem;
    color: var(--muted);
    font-size: 0.72rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .palette-item {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    border: none;
    background: transparent;
    color: var(--text);
    padding: 0.4rem 1rem;
    text-align: left;
    cursor: pointer;
  }

  .palette-item {
    transition: background var(--t-fast);
  }

  .palette-item:hover {
    background: var(--panel-soft);
  }

  .palette-item.selected {
    background: var(--accent-tint);
    box-shadow: inset 2px 0 0 var(--accent);
  }

  .palette-item.selected:hover {
    background: var(--accent-tint);
  }

  .item-main {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }

  .item-title {
    font-size: 0.9rem;
  }

  .item-api {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.72rem;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .item-shortcut {
    flex: none;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-sm);
    background: var(--panel-soft);
    color: var(--muted);
    padding: 0.1rem 0.4rem;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.72rem;
  }
</style>
