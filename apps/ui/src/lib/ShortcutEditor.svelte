<script lang="ts">
  import IconKeyboard from '~icons/lucide/keyboard';
  import IconX from '~icons/lucide/x';
  import IconSearch from '~icons/lucide/search';
  import IconRotateCcw from '~icons/lucide/rotate-ccw';
  import IconTriangleAlert from '~icons/lucide/triangle-alert';
  import KeyboardMap, { type KeyVisualState } from './KeyboardMap.svelte';
  import {
    KEY_MODES,
    KEYMAP_COMMANDS,
    KEYBOARD_ROWS,
    MODIFIER_CODES,
    chordEquals,
    chordLabel,
    getKeyBindings,
    type Chord,
    type KeyConflict,
    type KeyModeId,
    type KeymapCommand,
    type KeyScope
  } from './keybindings.svelte';

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  const store = getKeyBindings();

  type Plane = 'plain' | 'shift' | 'primary' | 'alt';
  const PLANES: readonly { id: Plane; label: string }[] = [
    { id: 'plain', label: 'Plain' },
    { id: 'shift', label: 'Shift' },
    { id: 'primary', label: 'Ctrl/Cmd' },
    { id: 'alt', label: 'Alt' }
  ];

  const SCOPE_LABELS: Record<KeyScope, string> = {
    editor: 'Editor',
    tierpane: 'Tiers',
    global: 'Global'
  };

  let plane = $state<Plane>('plain');
  let query = $state('');
  let selectedCode = $state<string | null>(null);
  let selectedCommandId = $state<string | null>(null);
  let listening = $state(false);
  let searchEl = $state<HTMLInputElement | null>(null);

  $effect(() => {
    searchEl?.focus();
  });

  function planeChord(code: string): Chord {
    return {
      code,
      primary: plane === 'primary',
      shift: plane === 'shift',
      alt: plane === 'alt'
    };
  }

  /** Every command bound to `chord` in the active mode, across every scope. */
  function commandsAt(chord: Chord): KeymapCommand[] {
    if (!store) return [];
    return KEYMAP_COMMANDS.filter((command) =>
      store.chordsFor(command.id).some((bound) => chordEquals(bound, chord))
    );
  }

  function keyState(code: string, label: string, width: number): KeyVisualState {
    if (MODIFIER_CODES.has(code)) {
      return {
        code,
        label,
        width,
        isModifier: true,
        assigned: false,
        conflicted: false,
        selected: false,
        title: label
      };
    }
    const chord = planeChord(code);
    const commands = commandsAt(chord);
    const byScope = new Map<KeyScope, number>();
    for (const command of commands) byScope.set(command.scope, (byScope.get(command.scope) ?? 0) + 1);
    const conflicted = [...byScope.values()].some((count) => count > 1);
    const title = commands.length
      ? commands.map((command) => `${command.title} (${SCOPE_LABELS[command.scope]})`).join(' · ')
      : 'Unassigned';
    return {
      code,
      label,
      width,
      isModifier: false,
      assigned: commands.length > 0,
      conflicted,
      selected: selectedCode === code,
      title
    };
  }

  const rows = $derived(
    KEYBOARD_ROWS.map((row) => row.map((key) => keyState(key.code, key.label, key.width ?? 1)))
  );

  const filteredCommands = $derived.by<KeymapCommand[]>(() => {
    const trimmed = query.trim().toLowerCase();
    if (!trimmed) return [...KEYMAP_COMMANDS];
    return KEYMAP_COMMANDS.filter((command) => command.title.toLowerCase().includes(trimmed));
  });

  const groupedCommands = $derived.by<{ group: string; items: KeymapCommand[] }[]>(() => {
    const out: { group: string; items: KeymapCommand[] }[] = [];
    for (const command of filteredCommands) {
      const last = out.at(-1);
      if (last && last.group === command.group) last.items.push(command);
      else out.push({ group: command.group, items: [command] });
    }
    return out;
  });

  const allConflicts = $derived<KeyConflict[]>(store ? store.conflicts() : []);

  const selectedCommand = $derived(
    selectedCommandId ? (KEYMAP_COMMANDS.find((command) => command.id === selectedCommandId) ?? null) : null
  );

  function conflictLabel(conflict: KeyConflict): string {
    const titles = conflict.commandIds
      .map((id) => KEYMAP_COMMANDS.find((command) => command.id === id)?.title ?? id)
      .join(', ');
    return `${chordLabel(conflict.chord)} — ${titles}`;
  }

  function selectCommand(id: string) {
    selectedCommandId = id;
    listening = false;
    const chord = store?.chordsFor(id)[0];
    if (!chord) {
      selectedCode = null;
      return;
    }
    selectedCode = chord.code;
    plane = chord.primary ? 'primary' : chord.shift ? 'shift' : chord.alt ? 'alt' : 'plain';
  }

  function selectKey(code: string) {
    selectedCode = code;
    listening = false;
    const commands = commandsAt(planeChord(code));
    selectedCommandId = commands[0]?.id ?? selectedCommandId;
  }

  function startRebind() {
    if (!selectedCommandId) return;
    listening = true;
  }

  function cancelRebind() {
    listening = false;
  }

  function clearBinding() {
    if (!selectedCommandId || !store) return;
    store.setOverride(selectedCommandId, []);
  }

  function resetBinding() {
    if (!selectedCommandId || !store) return;
    store.clearOverride(selectedCommandId);
  }

  function resetAll() {
    store?.resetMode();
  }

  // Captured in the capture phase and stopped there, so the shortcut editor's
  // own rebind step never also triggers the app's live shortcuts (EditorView,
  // TierPane, and the command palette all listen in the bubble phase).
  function onListenKeydown(event: KeyboardEvent) {
    if (!listening) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === 'Escape') {
      listening = false;
      return;
    }
    if (MODIFIER_CODES.has(event.code)) return;
    if (!selectedCommandId || !store) {
      listening = false;
      return;
    }
    const chord = planeChord(event.code);
    store.setOverride(selectedCommandId, [chord]);
    selectedCode = event.code;
    listening = false;
  }

  function onDialogKeydown(event: KeyboardEvent) {
    if (listening) return;
    if (event.key === 'Escape') {
      event.stopPropagation();
      onClose();
    }
  }

  function setMode(mode: KeyModeId) {
    store?.setMode(mode);
  }
</script>


<svelte:window onkeydowncapture={onListenKeydown} onkeydown={onDialogKeydown} />

<div
  class="backdrop"
  data-testid="shortcut-editor"
  role="presentation"
  onpointerdown={(event) => {
    if (event.target === event.currentTarget && !listening) onClose();
  }}
>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Keyboard shortcuts">
    <header class="head">
      <h2><IconKeyboard aria-hidden="true" />Keyboard shortcuts</h2>
      <button type="button" class="close" data-testid="shortcut-editor-close" aria-label="Close" onclick={onClose}>
        <IconX aria-hidden="true" />
      </button>
    </header>

    <div class="toolbar">
      <label class="search">
        <IconSearch aria-hidden="true" />
        <input
          bind:this={searchEl}
          type="text"
          placeholder="Search actions…"
          aria-label="Search actions"
          data-testid="shortcut-search"
          bind:value={query}
        />
      </label>

      <div class="modes" role="tablist" aria-label="Key mode">
        {#each KEY_MODES as mode (mode.id)}
          <button
            type="button"
            role="tab"
            class="mode-tab"
            class:active={store?.mode === mode.id}
            aria-selected={store?.mode === mode.id}
            data-testid={`shortcut-mode-${mode.id}`}
            title={mode.description}
            onclick={() => setMode(mode.id)}
          >
            {mode.label}
          </button>
        {/each}
      </div>
    </div>

    {#if allConflicts.length}
      <div class="conflicts" role="alert" data-testid="shortcut-conflicts">
        <IconTriangleAlert aria-hidden="true" />
        <ul>
          {#each allConflicts as conflict (conflictLabel(conflict))}
            <li>{conflictLabel(conflict)}</li>
          {/each}
        </ul>
      </div>
    {/if}

    <div class="body">
      <aside class="commands" data-testid="shortcut-command-list">
        {#each groupedCommands as section (section.group)}
          <div class="group-label">{section.group}</div>
          {#each section.items as command (command.id)}
            <button
              type="button"
              class="command-row"
              class:active={selectedCommandId === command.id}
              data-testid="shortcut-command"
              data-command-id={command.id}
              onclick={() => selectCommand(command.id)}
            >
              <span class="command-title">{command.title}</span>
              <span class="command-meta">
                {#if store?.isCustomized(command.id)}<span class="customized" title="Customized">●</span>{/if}
                <kbd>{store?.labelFor(command.id) || '—'}</kbd>
              </span>
            </button>
          {/each}
        {/each}
        {#if filteredCommands.length === 0}
          <p class="empty">No matching action.</p>
        {/if}
      </aside>

      <div class="board-wrap">
        <div class="planes" role="tablist" aria-label="Modifier plane">
          {#each PLANES as candidate (candidate.id)}
            <button
              type="button"
              role="tab"
              class="plane-tab"
              class:active={plane === candidate.id}
              aria-selected={plane === candidate.id}
              data-testid={`shortcut-plane-${candidate.id}`}
              onclick={() => (plane = candidate.id)}
            >
              {candidate.label}
            </button>
          {/each}
        </div>

        <div class="board-scroll">
          <KeyboardMap {rows} {selectedCode} onSelect={selectKey} />
        </div>

        <div class="detail" data-testid="shortcut-detail">
          {#if selectedCommand}
            <div class="detail-head">
              <span class="detail-title">{selectedCommand.title}</span>
              <span class="detail-scope">{SCOPE_LABELS[selectedCommand.scope]}</span>
            </div>
            <div class="detail-chord">
              {#if listening}
                <span class="listening" data-testid="shortcut-listening">Press a key, or Esc to cancel…</span>
              {:else}
                <kbd>{store?.labelFor(selectedCommand.id) || 'Unassigned'}</kbd>
              {/if}
            </div>
            <div class="detail-actions">
              <button type="button" data-testid="shortcut-rebind" onclick={startRebind} disabled={listening}>
                Rebind
              </button>
              <button type="button" data-testid="shortcut-clear" onclick={clearBinding} disabled={listening}>
                Clear
              </button>
              {#if store?.isCustomized(selectedCommand.id)}
                <button type="button" data-testid="shortcut-reset" onclick={resetBinding} disabled={listening}>
                  <IconRotateCcw aria-hidden="true" />Reset
                </button>
              {/if}
              {#if listening}
                <button type="button" onclick={cancelRebind}>Cancel</button>
              {/if}
            </div>
          {:else}
            <p class="detail-empty">Select an action or a key to see its binding.</p>
          {/if}
        </div>
      </div>
    </div>

    <footer class="foot">
      <button type="button" class="reset-all" data-testid="shortcut-reset-all" onclick={resetAll}>
        <IconRotateCcw aria-hidden="true" />Reset all to defaults
      </button>
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: color-mix(in oklab, #000 52%, transparent);
    backdrop-filter: blur(2px);
    z-index: 30;
    padding: 1.5rem;
  }

  .dialog {
    width: min(64rem, calc(100vw - 3rem));
    max-height: calc(100vh - 3rem);
    display: flex;
    flex-direction: column;
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-xl);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.7rem 1rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--panel-soft);
    flex: none;
  }

  .head h2 {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 0.45rem;
    font-size: 1rem;
    font-weight: 600;
  }

  .head h2 :global(svg) {
    font-size: 1.05rem;
    color: var(--accent);
  }

  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted);
    padding: 0.25rem;
    cursor: pointer;
  }

  .close:hover {
    background: var(--panel);
    color: var(--text);
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 0.6rem 1rem;
    border-bottom: 1px solid var(--chrome-strong);
    flex: none;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex: 1 1 auto;
    max-width: 20rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-md);
    background: var(--panel-soft);
    padding: 0.35rem 0.6rem;
  }

  .search :global(svg) {
    color: var(--muted);
    font-size: 0.95rem;
    flex: none;
  }

  .search input {
    flex: 1;
    border: none;
    background: transparent;
    color: var(--text);
    font-size: 0.85rem;
    outline: none;
  }

  .modes {
    display: flex;
    gap: 0.3rem;
  }

  .mode-tab {
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-md);
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.3rem 0.7rem;
    font-size: 0.8rem;
    cursor: pointer;
    transition:
      background var(--t-fast),
      border-color var(--t-fast),
      color var(--t-fast);
  }

  .mode-tab:hover {
    background: var(--panel);
  }

  .mode-tab.active {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }

  .conflicts {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.5rem 1rem;
    background: color-mix(in oklab, var(--danger) 12%, var(--panel));
    color: var(--danger);
    font-size: 0.8rem;
    border-bottom: 1px solid var(--chrome-strong);
    flex: none;
  }

  .conflicts :global(svg) {
    flex: none;
    margin-top: 0.15rem;
  }

  .conflicts ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .body {
    flex: 1 1 auto;
    min-height: 0;
    display: grid;
    grid-template-columns: 16rem minmax(0, 1fr);
  }

  .commands {
    overflow-y: auto;
    border-right: 1px solid var(--chrome-strong);
    padding: 0.4rem 0;
  }

  .group-label {
    padding: 0.5rem 0.85rem 0.2rem;
    color: var(--muted);
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .command-row {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    border: none;
    background: transparent;
    color: var(--text);
    padding: 0.35rem 0.85rem;
    text-align: left;
    cursor: pointer;
    font-size: 0.82rem;
    transition: background var(--t-fast);
  }

  .command-row:hover {
    background: var(--panel-soft);
  }

  .command-row.active {
    background: var(--accent-tint);
    box-shadow: inset 2px 0 0 var(--accent);
  }

  .command-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .command-meta {
    flex: none;
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .customized {
    color: var(--accent);
    font-size: 0.6rem;
  }

  .command-meta kbd {
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-sm);
    background: var(--panel-soft);
    color: var(--muted);
    padding: 0.05rem 0.35rem;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.68rem;
  }

  .empty {
    padding: 0.6rem 0.85rem;
    color: var(--muted);
    font-size: 0.82rem;
  }

  .board-wrap {
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    padding: 0.9rem 1rem;
    overflow-y: auto;
  }

  .planes {
    display: flex;
    gap: 0.3rem;
  }

  .plane-tab {
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-md);
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.3rem 0.7rem;
    font-size: 0.78rem;
    cursor: pointer;
    transition:
      background var(--t-fast),
      border-color var(--t-fast),
      color var(--t-fast);
  }

  .plane-tab:hover {
    background: var(--panel);
  }

  .plane-tab.active {
    background: var(--accent-tint);
    border-color: var(--accent);
    color: var(--accent-strong);
  }

  .board-scroll {
    overflow-x: auto;
    padding-bottom: 0.2rem;
  }

  .detail {
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-lg);
    background: var(--panel-soft);
    padding: 0.75rem 0.9rem;
  }

  .detail-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    margin-bottom: 0.4rem;
  }

  .detail-title {
    font-size: 0.9rem;
    font-weight: 600;
  }

  .detail-scope {
    font-size: 0.72rem;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .detail-chord {
    margin-bottom: 0.6rem;
  }

  .detail-chord kbd {
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-sm);
    background: var(--panel);
    color: var(--text);
    padding: 0.15rem 0.5rem;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.78rem;
  }

  .listening {
    color: var(--accent-strong);
    font-size: 0.82rem;
  }

  .detail-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .detail-actions button {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-md);
    background: var(--panel);
    color: var(--text);
    padding: 0.3rem 0.65rem;
    font-size: 0.78rem;
    cursor: pointer;
    transition:
      background var(--t-fast),
      border-color var(--t-fast);
  }

  .detail-actions button:hover:not(:disabled) {
    background: var(--accent-tint);
    border-color: color-mix(in oklab, var(--accent) 40%, var(--chrome-strong));
  }

  .detail-actions button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .detail-empty {
    margin: 0;
    color: var(--muted);
    font-size: 0.82rem;
  }

  .foot {
    display: flex;
    justify-content: flex-end;
    padding: 0.6rem 1rem;
    border-top: 1px solid var(--chrome-strong);
    flex: none;
  }

  .reset-all {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-md);
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.35rem 0.75rem;
    font-size: 0.8rem;
    cursor: pointer;
    transition:
      background var(--t-fast),
      border-color var(--t-fast);
  }

  .reset-all:hover {
    background: var(--panel);
    border-color: color-mix(in oklab, var(--accent) 32%, var(--chrome-strong));
  }
</style>
