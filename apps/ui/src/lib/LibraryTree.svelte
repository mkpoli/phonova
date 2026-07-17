<script lang="ts">
  import IconChevronRight from '~icons/lucide/chevron-right';
  import IconGripVertical from '~icons/lucide/grip-vertical';
  import IconFolder from '~icons/lucide/folder';
  import IconTrash from '~icons/lucide/trash-2';
  import IconTags from '~icons/lucide/tags';
  import IconInfo from '~icons/lucide/info';
  import IconUngroup from '~icons/lucide/ungroup';
  import WaveThumb from './WaveThumb.svelte';
  import InlineRename from './InlineRename.svelte';
  import { flattenTree, isGroup, mediaIdsOf, type LibraryRow } from './library';
  import { formatTime, type CoreClientLike, type LibraryNode, type RecordingEntry } from './types';

  interface Props {
    client: CoreClientLike | null;
    theme: 'light' | 'dark';
    /** The recordings behind the media leaves, by id. */
    recordings: RecordingEntry[];
    /** The tree to render (already filtered by any active search). */
    tree: LibraryNode[];
    collapsed: ReadonlySet<number>;
    /** Media ids removed but still inside the undo window: kept in the tree for
     * stable ordering, omitted from the visible rows. */
    hidden?: ReadonlySet<number>;
    /** The recording whose details panel is open, highlighted as selected. */
    selectedMediaId: number | null;
    onOpen: (mediaId: number) => void;
    onRenameRecording: (mediaId: number, name: string) => void;
    onDeleteRecording: (mediaId: number) => void;
    onShowDetails: (mediaId: number) => void;
    onToggleCollapse: (groupId: number) => void;
    onRenameGroup: (groupId: number, name: string) => void;
    onDissolveGroup: (groupId: number) => void;
    /** Reparents/reorders a node under `targetGroupId` (null = root) at `index`. */
    onMove: (key: string, targetGroupId: number | null, index: number) => void;
  }

  let {
    client,
    theme,
    recordings,
    tree,
    collapsed,
    hidden = new Set<number>(),
    selectedMediaId,
    onOpen,
    onRenameRecording,
    onDeleteRecording,
    onShowDetails,
    onToggleCollapse,
    onRenameGroup,
    onDissolveGroup,
    onMove
  }: Props = $props();

  const byId = $derived(new Map(recordings.map((r) => [r.mediaId, r])));
  const rows = $derived(
    flattenTree(tree, collapsed).filter((r) => isGroup(r.node) || !hidden.has(r.node.Media))
  );

  let focusKey = $state<string | null>(null);
  let renamers: Record<string, { edit: () => void }> = {};

  function sampleRateLabel(hz: number): string {
    return hz >= 1000 ? `${(hz / 1000).toFixed(hz % 1000 === 0 ? 0 : 1)} kHz` : `${hz} Hz`;
  }

  // Reordering reads the live tree, not the flattened (collapse-aware) rows, so a
  // move lands among every sibling including those hidden under a collapsed group.
  function siblingsOf(nodes: LibraryNode[], parentGroupId: number | null): LibraryNode[] {
    if (parentGroupId === null) return nodes;
    for (const node of nodes) {
      if (isGroup(node)) {
        if (node.Group.id === parentGroupId) return node.Group.children;
        const found = siblingsOf(node.Group.children, parentGroupId);
        if (found.length || containsGroup(node.Group.children, parentGroupId)) return found;
      }
    }
    return [];
  }

  function containsGroup(nodes: LibraryNode[], groupId: number): boolean {
    return nodes.some(
      (n) => isGroup(n) && (n.Group.id === groupId || containsGroup(n.Group.children, groupId))
    );
  }

  // --- Keyboard treegrid navigation ---

  function focusRow(key: string | null) {
    if (!key) return;
    focusKey = key;
    queueMicrotask(() => {
      const el = document.querySelector<HTMLElement>(`[data-testid="tree-row"][data-key="${key}"]`);
      el?.focus();
    });
  }

  function moveFocus(delta: number) {
    const idx = rows.findIndex((r) => r.key === focusKey);
    const next = Math.max(0, Math.min(rows.length - 1, (idx < 0 ? 0 : idx) + delta));
    focusRow(rows[next]?.key ?? null);
  }

  function reorder(row: LibraryRow, delta: number) {
    const siblings = siblingsOf(tree, row.parentGroupId);
    const target = row.index + delta;
    if (target < 0 || target >= siblings.length) return;
    onMove(row.key, row.parentGroupId, target);
    focusRow(row.key);
  }

  function handleRowKeydown(event: KeyboardEvent, row: LibraryRow) {
    // Text entry (the inline rename field) owns its own keys.
    if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) {
      return;
    }
    const group = isGroup(row.node) ? row.node.Group : null;
    const media = isGroup(row.node) ? null : row.node.Media;
    if (event.altKey && (event.key === 'ArrowUp' || event.key === 'ArrowDown')) {
      event.preventDefault();
      reorder(row, event.key === 'ArrowDown' ? 1 : -1);
      return;
    }
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault();
        moveFocus(1);
        break;
      case 'ArrowUp':
        event.preventDefault();
        moveFocus(-1);
        break;
      case 'ArrowRight':
        if (group && collapsed.has(group.id)) {
          event.preventDefault();
          onToggleCollapse(group.id);
        }
        break;
      case 'ArrowLeft':
        if (group && !collapsed.has(group.id)) {
          event.preventDefault();
          onToggleCollapse(group.id);
        }
        break;
      case 'Enter':
        event.preventDefault();
        if (group) onToggleCollapse(group.id);
        else if (media !== null) onOpen(media);
        break;
      case 'F2':
        event.preventDefault();
        renamers[row.key]?.edit();
        break;
      case 'Delete':
      case 'Backspace':
        if (media !== null) {
          event.preventDefault();
          onDeleteRecording(media);
        }
        break;
    }
  }

  // --- Pointer drag: reparent and reorder ---

  let dragKey = $state<string | null>(null);
  let dropKey = $state<string | null>(null);
  let dropPos = $state<'before' | 'after' | 'inside'>('before');

  function beginDrag(event: PointerEvent, key: string) {
    event.preventDefault();
    dragKey = key;
    // The move and up are tracked on `window`, so no pointer capture is needed;
    // capture would retarget events in ways synthesized input handles unevenly.
  }

  function rowUnderPoint(x: number, y: number): HTMLElement | null {
    const el = document.elementFromPoint(x, y);
    return el?.closest<HTMLElement>('[data-testid="tree-row"]') ?? null;
  }

  function handlePointerMove(event: PointerEvent) {
    if (!dragKey) return;
    const el = rowUnderPoint(event.clientX, event.clientY);
    if (!el) {
      dropKey = '__root__';
      dropPos = 'after';
      return;
    }
    const key = el.dataset.key ?? null;
    const rect = el.getBoundingClientRect();
    const frac = (event.clientY - rect.top) / rect.height;
    const targetIsGroup = key?.startsWith('group:') ?? false;
    dropKey = key;
    if (targetIsGroup && frac > 0.25 && frac < 0.75) dropPos = 'inside';
    else dropPos = frac < 0.5 ? 'before' : 'after';
  }

  function handlePointerUp() {
    if (!dragKey || !dropKey) return resetDrag();
    if (dropKey === dragKey) return resetDrag();
    if (dropKey === '__root__') {
      onMove(dragKey, null, tree.length);
      return resetDrag();
    }
    const target = rows.find((r) => r.key === dropKey);
    if (!target) return resetDrag();
    if (dropPos === 'inside' && isGroup(target.node)) {
      onMove(dragKey, target.node.Group.id, 0);
    } else {
      const index = dropPos === 'after' ? target.index + 1 : target.index;
      onMove(dragKey, target.parentGroupId, index);
    }
    resetDrag();
  }

  function resetDrag() {
    dragKey = null;
    dropKey = null;
  }
</script>

<svelte:window onpointermove={handlePointerMove} onpointerup={handlePointerUp} />

<div class="tree" role="treegrid" aria-label="Corpus library" data-testid="library-tree">
  <div class="head" role="row">
    <span class="c-handle" aria-hidden="true"></span>
    <span class="c-main" role="columnheader">Name</span>
    <span class="c-num" role="columnheader">Duration</span>
    <span class="c-num" role="columnheader">Sample rate</span>
    <span class="c-num" role="columnheader">Channels</span>
    <span class="c-annot" role="columnheader">Annotation</span>
    <span class="c-actions" aria-hidden="true"></span>
  </div>

  {#each rows as row (row.key)}
    {@const group = isGroup(row.node) ? row.node.Group : null}
    {@const rec = isGroup(row.node) ? null : byId.get(row.node.Media)}
    <div
      class="row"
      class:group={!!group}
      class:selected={!group && rec?.mediaId === selectedMediaId}
      class:dragging={dragKey === row.key}
      class:drop-before={dropKey === row.key && dropPos === 'before'}
      class:drop-after={dropKey === row.key && dropPos === 'after'}
      class:drop-inside={dropKey === row.key && dropPos === 'inside'}
      data-testid="tree-row"
      data-key={row.key}
      data-depth={row.depth}
      role="row"
      aria-level={row.depth + 1}
      aria-expanded={group ? !collapsed.has(group.id) : undefined}
      tabindex={focusKey === row.key || (focusKey === null && row === rows[0]) ? 0 : -1}
      onkeydown={(event) => handleRowKeydown(event, row)}
      onfocus={() => (focusKey = row.key)}
    >
      <button
        type="button"
        class="handle"
        aria-label="Drag to reorder"
        title="Drag to reorder"
        data-testid="tree-drag"
        onpointerdown={(event) => beginDrag(event, row.key)}
      >
        <IconGripVertical aria-hidden="true" />
      </button>

      {#if group}
        <div class="main" style="padding-left: {row.depth * 1.15}rem">
          <button
            type="button"
            class="disclose"
            data-testid="tree-disclose"
            aria-label={collapsed.has(group.id) ? 'Expand group' : 'Collapse group'}
            aria-expanded={!collapsed.has(group.id)}
            onclick={() => onToggleCollapse(group.id)}
          >
            <span class="chev" class:open={!collapsed.has(group.id)}><IconChevronRight aria-hidden="true" /></span>
          </button>
          <IconFolder class="folder" aria-hidden="true" />
          <InlineRename
            bind:this={renamers[row.key]}
            name={group.name}
            class="group-name"
            label="Rename group"
            testId="rename-group"
            onRename={(next) => onRenameGroup(group.id, next)}
          />
          <span class="count" data-testid="group-count">
            {mediaIdsOf(group.children).length}
          </span>
        </div>
        <span class="c-num"></span>
        <span class="c-num"></span>
        <span class="c-num"></span>
        <span class="c-annot"></span>
        <div class="c-actions">
          <button
            type="button"
            class="act"
            aria-label="Dissolve group"
            title="Dissolve group"
            data-testid="dissolve-group"
            onclick={() => onDissolveGroup(group.id)}
          >
            <IconUngroup aria-hidden="true" />
          </button>
        </div>
      {:else if rec}
        <div
          class="main open"
          data-testid="corpus-row"
          data-recording-name={rec.name}
          data-has-annotation={rec.hasAnnotation}
          role="button"
          tabindex="-1"
          style="padding-left: {row.depth * 1.15}rem"
          onclick={(event) => {
            if (event.target instanceof Element && event.target.closest('.inline-rename')) return;
            onOpen(rec.mediaId);
          }}
          onkeydown={(event) => {
            if (event.target !== event.currentTarget) return;
            if (event.key === 'Enter' || event.key === ' ') {
              event.preventDefault();
              onOpen(rec.mediaId);
            }
          }}
        >
          <span class="disclose-spacer" aria-hidden="true"></span>
          <span class="thumb-box">
            <WaveThumb {client} audioId={rec.audioId} duration={rec.duration} {theme} width={96} height={34} />
          </span>
          <InlineRename
            bind:this={renamers[row.key]}
            name={rec.name}
            class="rec-name"
            label="Rename recording"
            testId="rename-corpus"
            onRename={(next) => onRenameRecording(rec.mediaId, next)}
          />
        </div>
        <span class="c-num">{formatTime(rec.duration)}</span>
        <span class="c-num">{sampleRateLabel(rec.sampleRate)}</span>
        <span class="c-num">{rec.channels}</span>
        <span class="c-annot">
          {#if rec.hasAnnotation}
            <span class="tag" data-testid="annotation-present"><IconTags aria-hidden="true" />tiers</span>
          {:else}
            <span class="tag muted">—</span>
          {/if}
        </span>
        <div class="c-actions">
          <button
            type="button"
            class="act"
            aria-label="Recording details"
            title="Recording details"
            data-testid="row-details"
            onclick={() => onShowDetails(rec.mediaId)}
          >
            <IconInfo aria-hidden="true" />
          </button>
          <button
            type="button"
            class="act danger"
            aria-label="Remove recording"
            title="Remove recording"
            data-testid="row-delete"
            onclick={() => onDeleteRecording(rec.mediaId)}
          >
            <IconTrash aria-hidden="true" />
          </button>
        </div>
      {/if}
    </div>
  {/each}
</div>

<style>
  .tree {
    width: 100%;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-lg);
    overflow: hidden;
    background: var(--panel);
    box-shadow: var(--shadow-sm);
    font-size: 0.9rem;
  }

  .head,
  .row {
    display: grid;
    grid-template-columns: 1.4rem minmax(0, 1fr) 6rem 6.5rem 5rem 7rem 4.5rem;
    align-items: center;
  }

  .head {
    padding: 0.5rem 0.6rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--panel-soft);
    color: var(--muted);
    font-size: 0.68rem;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    position: sticky;
    top: 0;
    z-index: 1;
  }

  .head span {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .head .c-num,
  .head .c-annot {
    font-weight: 500;
  }

  .c-num {
    text-align: right;
    font-variant-numeric: tabular-nums;
    padding-right: 0.35rem;
  }

  .row {
    position: relative;
    padding: 0.35rem 0.6rem;
    border-bottom: 1px solid color-mix(in oklab, var(--chrome-strong) 65%, transparent);
    transition: background var(--t-fast);
  }

  .row:last-child {
    border-bottom: none;
  }

  .row:hover {
    background: var(--accent-tint);
  }

  .row:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .row.selected {
    background: var(--accent-tint);
    box-shadow: inset 3px 0 0 var(--accent);
  }

  .row.dragging {
    opacity: 0.5;
  }

  .row.drop-before::before,
  .row.drop-after::after {
    content: '';
    position: absolute;
    left: 1.6rem;
    right: 0.6rem;
    height: 2px;
    background: var(--accent);
  }

  .row.drop-before::before {
    top: -1px;
  }

  .row.drop-after::after {
    bottom: -1px;
  }

  .row.drop-inside {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
    background: var(--accent-tint);
  }

  .handle {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    color: var(--muted);
    padding: 0.1rem;
    cursor: grab;
    opacity: 0;
    touch-action: none;
    transition: opacity var(--t-fast);
  }

  .row:hover .handle,
  .row:focus-within .handle {
    opacity: 1;
  }

  .handle :global(svg) {
    font-size: 0.9rem;
  }

  .main {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
  }

  .main.open {
    cursor: pointer;
  }

  .disclose {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    color: var(--muted);
    padding: 0.1rem;
    flex: none;
  }

  .chev {
    display: inline-flex;
    transition: transform var(--t-fast);
  }

  .chev.open {
    transform: rotate(90deg);
  }

  .disclose-spacer {
    width: 1.1rem;
    flex: none;
  }

  .main :global(.folder) {
    color: var(--accent-strong);
    flex: none;
  }

  :global(.group-name) {
    font-weight: 600;
  }

  .count {
    flex: none;
    font-size: 0.74rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    padding: 0.05rem 0.4rem;
    border-radius: 999px;
    background: var(--panel-soft);
  }

  .thumb-box {
    flex: none;
    width: 96px;
    height: 34px;
    line-height: 0;
  }

  .thumb-box :global(.thumb) {
    border: 1px solid var(--chrome-strong);
    box-shadow: var(--shadow-sm);
  }

  :global(.rec-name) {
    font-weight: 500;
    min-width: 0;
  }

  .c-annot {
    display: flex;
  }

  .tag {
    display: inline-flex;
    align-items: center;
    gap: 0.28rem;
    padding: 0.12rem 0.55rem;
    border-radius: 999px;
    background: var(--accent-tint);
    color: var(--accent-strong);
    font-size: 0.76rem;
    border: 1px solid color-mix(in oklab, var(--accent) 30%, transparent);
  }

  .tag :global(svg) {
    font-size: 0.85rem;
  }

  .tag.muted {
    background: transparent;
    color: var(--muted);
    border-color: transparent;
  }

  .c-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.15rem;
  }

  .act {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted);
    padding: 0.22rem;
    opacity: 0;
    transition:
      opacity var(--t-fast),
      color var(--t-fast),
      background var(--t-fast),
      border-color var(--t-fast);
  }

  .row:hover .act,
  .row:focus-within .act {
    opacity: 1;
  }

  .act:hover {
    color: var(--text);
    background: var(--panel-soft);
    border-color: var(--chrome-strong);
  }

  .act.danger:hover {
    color: var(--danger);
    border-color: color-mix(in oklab, var(--danger) 45%, transparent);
  }

  .act :global(svg) {
    font-size: 0.9rem;
  }
</style>
