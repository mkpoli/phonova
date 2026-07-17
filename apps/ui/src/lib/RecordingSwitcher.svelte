<script lang="ts">
  import IconChevronDown from '~icons/lucide/chevron-down';
  import IconTags from '~icons/lucide/tags';
  import IconSearch from '~icons/lucide/search';
  import WaveThumb from './WaveThumb.svelte';
  import InlineRename from './InlineRename.svelte';
  import { isGroup } from './library';
  import { formatTime, type AudioId, type CoreClientLike, type LibraryNode } from './types';

  interface SwitcherRecording {
    mediaId: number;
    name: string;
    duration: number;
    audioId: AudioId | null;
    hasAnnotation: boolean;
  }

  interface Props {
    client: CoreClientLike | null;
    theme: 'light' | 'dark';
    recordings: SwitcherRecording[];
    currentRecordingId: number | null;
    /** The library tree, so the popover mirrors the corpus's grouping. Flat when absent. */
    groups?: LibraryNode[];
    onSwitch: (mediaId: number) => void;
    onRename?: (mediaId: number, name: string) => void;
  }

  let { client, theme, recordings, currentRecordingId, groups, onSwitch, onRename }: Props =
    $props();

  type Entry =
    | { kind: 'header'; id: string; name: string; depth: number }
    | { kind: 'option'; rec: SwitcherRecording; depth: number };

  let open = $state(false);
  let query = $state('');
  let activeIndex = $state(0);
  let rootEl = $state<HTMLDivElement | null>(null);
  let triggerEl = $state<HTMLButtonElement | null>(null);
  let inputEl = $state<HTMLInputElement | null>(null);
  let listEl = $state<HTMLUListElement | null>(null);

  const byId = $derived(new Map(recordings.map((r) => [r.mediaId, r])));

  const current = $derived(
    recordings.find((r) => r.mediaId === currentRecordingId) ?? recordings[0] ?? null
  );

  // Ordered header/option entries following the library tree. Root recordings
  // list without a header; a group contributes a header then its members, so a
  // project with no groups renders a flat list.
  const entries = $derived.by<Entry[]>(() => {
    const out: Entry[] = [];
    const tree: LibraryNode[] | undefined = groups;
    if (tree && tree.some((node) => isGroup(node))) {
      const walk = (nodes: LibraryNode[], depth: number) => {
        for (const node of nodes) {
          if (isGroup(node)) {
            out.push({ kind: 'header', id: `group:${node.Group.id}`, name: node.Group.name, depth });
            walk(node.Group.children, depth + 1);
          } else {
            const rec = byId.get(node.Media);
            if (rec) out.push({ kind: 'option', rec, depth });
          }
        }
      };
      walk(tree, 0);
      // Recordings absent from the tree (a freshly added take) still list, so
      // the switcher never hides a recording the corpus can open.
      const listed = new Set(out.filter((e) => e.kind === 'option').map((e) => (e as { rec: SwitcherRecording }).rec.mediaId));
      for (const rec of recordings) if (!listed.has(rec.mediaId)) out.push({ kind: 'option', rec, depth: 0 });
      return out;
    }
    return recordings.map((rec) => ({ kind: 'option', rec, depth: 0 }) as Entry);
  });

  // Filter options by name, then drop headers left with no visible member.
  const visibleEntries = $derived.by<Entry[]>(() => {
    const needle = query.trim().toLowerCase();
    const matched = entries.filter(
      (e) => e.kind === 'header' || e.rec.name.toLowerCase().includes(needle)
    );
    return matched.filter((e, i) => {
      if (e.kind !== 'header') return true;
      for (let j = i + 1; j < matched.length; j += 1) {
        const f = matched[j];
        if (f.kind === 'header' && f.depth <= e.depth) break;
        if (f.kind === 'option') return true;
      }
      return false;
    });
  });

  const options = $derived(
    visibleEntries.filter((e): e is Extract<Entry, { kind: 'option' }> => e.kind === 'option')
  );

  const optionId = (mediaId: number) => `switcher-opt-${mediaId}`;

  $effect(() => {
    if (activeIndex > options.length - 1) activeIndex = Math.max(0, options.length - 1);
  });

  // Keep the highlighted row in view as arrows walk the list.
  $effect(() => {
    if (!open) return;
    const active = options[activeIndex];
    if (!active) return;
    void query;
    queueMicrotask(() => {
      listEl?.querySelector<HTMLElement>(`#${CSS.escape(optionId(active.rec.mediaId))}`)?.scrollIntoView({ block: 'nearest' });
    });
  });

  export function show() {
    if (open) return;
    query = '';
    const idx = options.findIndex((o) => o.rec.mediaId === currentRecordingId);
    activeIndex = idx >= 0 ? idx : 0;
    open = true;
    queueMicrotask(() => inputEl?.focus());
  }

  function hide(returnFocus = true) {
    if (!open) return;
    open = false;
    if (returnFocus) queueMicrotask(() => triggerEl?.focus());
  }

  function toggle() {
    if (open) hide();
    else show();
  }

  function choose(mediaId: number) {
    hide();
    if (mediaId !== currentRecordingId) onSwitch(mediaId);
  }

  function onInputKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      hide();
      return;
    }
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      if (options.length) activeIndex = (activeIndex + 1) % options.length;
      return;
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      if (options.length) activeIndex = (activeIndex - 1 + options.length) % options.length;
      return;
    }
    if (event.key === 'Home') {
      event.preventDefault();
      activeIndex = 0;
      return;
    }
    if (event.key === 'End') {
      event.preventDefault();
      activeIndex = Math.max(0, options.length - 1);
      return;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      const active = options[activeIndex];
      if (active) choose(active.rec.mediaId);
    }
  }

  function onWindowPointerDown(event: PointerEvent) {
    if (!open) return;
    if (rootEl && event.target instanceof Node && !rootEl.contains(event.target)) hide(false);
  }

  const activeMediaId = $derived(options[activeIndex]?.rec.mediaId ?? null);
</script>

<svelte:window onpointerdown={onWindowPointerDown} />

<div class="switcher" bind:this={rootEl}>
  <button
    type="button"
    class="trigger"
    bind:this={triggerEl}
    data-testid="recording-switcher"
    aria-haspopup="listbox"
    aria-expanded={open}
    onclick={toggle}
  >
    <span class="trigger-name" data-testid="recording-switcher-name">{current?.name ?? ''}</span>
    <span class="chev" class:open aria-hidden="true"><IconChevronDown /></span>
  </button>

  {#if open}
    <div class="popover" data-testid="recording-switcher-popover">
      <div class="search">
        <IconSearch class="search-icon" aria-hidden="true" />
        <input
          bind:this={inputEl}
          class="search-input"
          data-testid="recording-switcher-search"
          type="text"
          role="combobox"
          aria-expanded="true"
          aria-controls="recording-switcher-list"
          aria-activedescendant={activeMediaId !== null ? optionId(activeMediaId) : undefined}
          aria-autocomplete="list"
          aria-label="Switch recording"
          placeholder="Filter recordings…"
          bind:value={query}
          onkeydown={onInputKeydown}
        />
      </div>

      <ul
        bind:this={listEl}
        id="recording-switcher-list"
        class="list"
        role="listbox"
        aria-label="Recordings"
      >
        {#if options.length === 0}
          <li class="empty" data-testid="recording-switcher-empty" role="presentation">
            No recording matches.
          </li>
        {/if}
        {#each visibleEntries as entry (entry.kind === 'header' ? entry.id : `media:${entry.rec.mediaId}`)}
          {#if entry.kind === 'header'}
            <li
              class="group"
              role="presentation"
              data-testid="switcher-group"
              style="padding-left: {0.7 + entry.depth * 0.9}rem"
            >
              {entry.name}
            </li>
          {:else}
            {@const rec = entry.rec}
            {@const active = rec.mediaId === activeMediaId}
            <!-- Options are driven from the combobox input via aria-activedescendant; the
                 input owns arrow/Enter, so an option needs no key handler of its own. -->
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <li
              id={optionId(rec.mediaId)}
              class="option"
              class:active
              class:current={rec.mediaId === currentRecordingId}
              role="option"
              aria-selected={rec.mediaId === currentRecordingId}
              data-testid="switcher-option"
              data-media-id={rec.mediaId}
              data-active={active}
              data-current={rec.mediaId === currentRecordingId}
              style="padding-left: {0.5 + entry.depth * 0.9}rem"
              onpointermove={() => (activeIndex = options.findIndex((o) => o.rec.mediaId === rec.mediaId))}
              onclick={(event) => {
                if (event.target instanceof Element && event.target.closest('.inline-rename')) return;
                choose(rec.mediaId);
              }}
            >
              <span class="thumb-box">
                <WaveThumb {client} audioId={rec.audioId} duration={rec.duration} {theme} width={96} height={30} />
              </span>
              <span class="name-cell">
                {#if onRename}
                  <InlineRename
                    name={rec.name}
                    class="opt-name"
                    label="Rename recording"
                    testId={rec.mediaId === currentRecordingId ? 'rename-recording' : `rename-switcher-${rec.mediaId}`}
                    onRename={(next) => onRename?.(rec.mediaId, next)}
                  />
                {:else}
                  <span class="opt-name">{rec.name}</span>
                {/if}
              </span>
              <span class="duration">{formatTime(rec.duration)}</span>
              <span class="annot">
                {#if rec.hasAnnotation}
                  <span class="tag"><IconTags aria-hidden="true" />tiers</span>
                {:else}
                  <span class="tag muted">—</span>
                {/if}
              </span>
            </li>
          {/if}
        {/each}
      </ul>
    </div>
  {/if}
</div>

<style>
  .switcher {
    position: relative;
    display: inline-flex;
    min-width: 0;
  }

  .trigger {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    max-width: 20rem;
    min-height: 1.6rem;
    padding: 0.2rem 0.5rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-sm);
    background: var(--panel-soft);
    color: var(--text);
    font: inherit;
    transition:
      background var(--t-fast),
      border-color var(--t-fast);
  }

  .trigger:hover,
  .trigger[aria-expanded='true'] {
    background: var(--panel);
    border-color: color-mix(in oklab, var(--accent) 32%, var(--chrome-strong));
  }

  .trigger-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 500;
  }

  .chev {
    display: inline-flex;
    flex: none;
    color: var(--muted);
    transition: transform var(--t-fast);
  }

  .chev :global(svg) {
    font-size: 0.9rem;
  }

  .chev.open {
    transform: rotate(180deg);
  }

  .popover {
    position: absolute;
    top: calc(100% + 0.35rem);
    left: 0;
    z-index: 30;
    width: min(24rem, calc(100vw - 2rem));
    max-height: min(60vh, 26rem);
    display: flex;
    flex-direction: column;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-xl);
    background: var(--panel);
    color: var(--text);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0 0.75rem;
    border-bottom: 1px solid var(--chrome-strong);
  }

  .search :global(.search-icon) {
    flex: none;
    font-size: 0.95rem;
    color: var(--muted);
  }

  .search-input {
    flex: 1;
    min-width: 0;
    border: none;
    background: transparent;
    color: var(--text);
    padding: 0.55rem 0;
    font-size: 0.85rem;
    outline: none;
  }

  .list {
    margin: 0;
    padding: 0.3rem 0;
    list-style: none;
    overflow-y: auto;
  }

  .empty {
    padding: 0.7rem 0.85rem;
    color: var(--muted);
    font-size: 0.84rem;
  }

  .group {
    padding: 0.45rem 0.7rem 0.2rem;
    color: var(--muted);
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .option {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 0.6rem;
    padding: 0.3rem 0.7rem;
    cursor: pointer;
    transition: background var(--t-fast);
  }

  .option:hover {
    background: var(--panel-soft);
  }

  .option.active {
    background: var(--accent-tint);
    box-shadow: inset 2px 0 0 var(--accent);
  }

  .option.current .opt-name {
    font-weight: 600;
  }

  .thumb-box {
    flex: none;
    width: 96px;
    height: 30px;
    line-height: 0;
  }

  .thumb-box :global(.thumb) {
    border: 1px solid var(--chrome-strong);
  }

  .name-cell {
    min-width: 0;
    display: flex;
    align-items: center;
  }

  :global(.opt-name) {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.85rem;
  }

  .duration {
    flex: none;
    color: var(--muted);
    font-size: 0.78rem;
    font-variant-numeric: tabular-nums;
  }

  .annot {
    flex: none;
    display: flex;
  }

  .tag {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.1rem 0.45rem;
    border-radius: 999px;
    background: var(--accent-tint);
    color: var(--accent-strong);
    font-size: 0.7rem;
    border: 1px solid color-mix(in oklab, var(--accent) 30%, transparent);
  }

  .tag :global(svg) {
    font-size: 0.8rem;
  }

  .tag.muted {
    background: transparent;
    color: var(--muted);
    border-color: transparent;
  }
</style>
