<script lang="ts">
  import IconArrowLeft from '~icons/lucide/arrow-left';
  import IconSave from '~icons/lucide/save';
  import IconMic from '~icons/lucide/mic';
  import IconSun from '~icons/lucide/sun';
  import IconMoon from '~icons/lucide/moon';
  import IconFolderOpen from '~icons/lucide/folder-open';
  import IconFolderPlus from '~icons/lucide/folder-plus';
  import IconInfo from '~icons/lucide/info';
  import IconSearch from '~icons/lucide/search';
  import IconRotateCcw from '~icons/lucide/rotate-ccw';
  import IconPackage from '~icons/lucide/package';
  import LibraryTree from './LibraryTree.svelte';
  import MetadataPanel from './MetadataPanel.svelte';
  import ProjectExportDialog from './ProjectExportDialog.svelte';
  import { filesFromDataTransfer } from './dnd';
  import { registerCommands } from './commands.svelte';
  import { flatLibrary, filterTree } from './library';
  import {
    type CoreClientLike,
    type LibraryNode,
    type ProjectExportMode,
    type RecordingEntry
  } from './types';

  interface Metadata {
    description: string;
    authors: string[];
    tags: string[];
  }

  interface Props {
    client: CoreClientLike | null;
    projectName: string;
    recordings: RecordingEntry[];
    theme: 'light' | 'dark';
    busy: boolean;
    busyLabel: string;
    dirty: boolean;
    /** Last container write time; advances as edits persist. */
    savedAt?: number;
    onOpenRecording: (recording: RecordingEntry) => void;
    onImportFiles: (files: File[]) => void;
    onBack: () => void;
    onSave: () => void;
    onThemeChange: (theme: 'light' | 'dark') => void;
    /** Starts a microphone recording; absent when the browser cannot capture. */
    onStartRecording?: () => void;
    /** Whether a take is currently being captured. */
    recording?: boolean;

    // --- Library management (the web shell wires these; desktop omits them and
    // the corpus falls back to a flat, read-only listing). ---

    /** The ordered library tree; a flat listing when omitted. */
    groups?: LibraryNode[];
    /** Ids of collapsed groups, persisted in the project's view state. */
    collapsed?: number[];
    onToggleCollapse?: (groupId: number) => void;
    onCreateGroup?: () => void;
    onRenameGroup?: (groupId: number, name: string) => void;
    onDissolveGroup?: (groupId: number) => void;
    onMoveNode?: (key: string, targetGroupId: number | null, index: number) => void;
    onRenameRecording?: (mediaId: number, name: string) => void;
    onDeleteRecording?: (mediaId: number) => void;
    /** Exports the whole project as a `.phxproj`; absent hides the export action. */
    onExportProject?: (mode: ProjectExportMode) => void;
    /** Exports one recording's whole audio as a WAV; absent hides the row action. */
    onExportRecording?: (mediaId: number) => void;
    onUpdateRecordingMetadata?: (mediaId: number, metadata: Metadata) => void;
    onUpdateProjectMetadata?: (metadata: Metadata) => void;
    projectDescription?: string;
    projectAuthors?: string[];
    projectTags?: string[];
    /** Media ids removed within the still-open undo window. */
    pendingRemovals?: number[];
    /**
     * The most recent removal offered for undo, or null. `stale` means the
     * journal has moved on since the delete — undoing is no longer safe, so
     * the banner switches to a manual-history message and disables the
     * button rather than risk undoing an unrelated later edit.
     */
    removalUndo?: { name: string; stale?: boolean } | null;
    onUndoRemoval?: () => void;
  }

  let {
    client,
    projectName,
    recordings,
    theme,
    busy,
    busyLabel,
    dirty,
    savedAt,
    onOpenRecording,
    onImportFiles,
    onBack,
    onSave,
    onThemeChange,
    onStartRecording,
    recording = false,
    groups,
    collapsed = [],
    onToggleCollapse,
    onCreateGroup,
    onRenameGroup,
    onDissolveGroup,
    onMoveNode,
    onRenameRecording,
    onDeleteRecording,
    onExportProject,
    onExportRecording,
    onUpdateRecordingMetadata,
    onUpdateProjectMetadata,
    projectDescription = '',
    projectAuthors = [],
    projectTags = [],
    pendingRemovals = [],
    removalUndo = null,
    onUndoRemoval
  }: Props = $props();

  let dragging = $state(false);
  let fileInput = $state<HTMLInputElement | null>(null);
  let exportOpen = $state(false);

  // The details inspector: the project, one recording, or closed.
  let details = $state<{ scope: 'project' } | { scope: 'recording'; mediaId: number } | null>(null);

  // Search over recording name, tags, and annotation labels.
  let query = $state('');
  let labelMatches = $state<Set<number>>(new Set());

  const byId = $derived(new Map(recordings.map((r) => [r.mediaId, r])));
  const hidden = $derived(new Set(pendingRemovals));
  const collapsedSet = $derived(new Set(collapsed));

  const baseTree = $derived(groups ?? flatLibrary(recordings.map((r) => r.mediaId)));

  // The visible media set after search. An empty query shows everything.
  const visible = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return null;
    const keep = new Set<number>();
    for (const rec of recordings) {
      if (hidden.has(rec.mediaId)) continue;
      const nameHit = rec.name.toLowerCase().includes(q);
      const tagHit = rec.tags.some((t) => t.toLowerCase().includes(q));
      if (nameHit || tagHit || labelMatches.has(rec.mediaId)) keep.add(rec.mediaId);
    }
    return keep;
  });

  // When a search is active the tree is filtered to matching recordings and
  // their enclosing groups, and collapse is ignored so every hit shows.
  const tree = $derived(visible ? filterTree(baseTree, visible) : baseTree);
  const effectiveCollapsed = $derived(visible ? new Set<number>() : collapsedSet);

  const visibleCount = $derived(
    recordings.filter((r) => !hidden.has(r.mediaId) && (!visible || visible.has(r.mediaId))).length
  );

  // Annotation-label search runs through the engine, debounced, and resolves to
  // the recordings whose documents carry a matching label.
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  $effect(() => {
    const q = query.trim();
    if (searchTimer) clearTimeout(searchTimer);
    if (!q || !client) {
      labelMatches = new Set();
      return;
    }
    const current = client;
    searchTimer = setTimeout(async () => {
      try {
        const hits = await current.searchLabels(q, false);
        const annToMedia = new Map<string, number>();
        for (const rec of recordings) {
          if (rec.annotationId !== null) annToMedia.set(String(rec.annotationId), rec.mediaId);
        }
        const next = new Set<number>();
        for (const hit of hits) {
          const mediaId = annToMedia.get(String(hit.annotation));
          if (mediaId !== undefined) next.add(mediaId);
        }
        labelMatches = next;
      } catch {
        labelMatches = new Set();
      }
    }, 180);
  });

  const detailsSubject = $derived.by(() => {
    if (!details) return null;
    if (details.scope === 'project') {
      return {
        key: 'project',
        scope: 'project' as const,
        title: projectName,
        description: projectDescription,
        authors: projectAuthors,
        tags: projectTags
      };
    }
    const rec = byId.get(details.mediaId);
    if (!rec) return null;
    return {
      key: `recording:${rec.mediaId}`,
      scope: 'recording' as const,
      title: rec.name,
      description: rec.description,
      authors: rec.authors,
      tags: rec.tags
    };
  });

  const selectedMediaId = $derived(details?.scope === 'recording' ? details.mediaId : null);

  function showRecordingDetails(mediaId: number) {
    details = { scope: 'recording', mediaId };
  }

  function saveDetails(metadata: Metadata) {
    if (!details) return;
    if (details.scope === 'project') onUpdateProjectMetadata?.(metadata);
    else onUpdateRecordingMetadata?.(details.mediaId, metadata);
  }

  async function handleDrop(event: DragEvent) {
    event.preventDefault();
    dragging = false;
    if (!event.dataTransfer) return;
    const files = await filesFromDataTransfer(event.dataTransfer);
    if (files.length > 0) onImportFiles(files);
  }

  function handleInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const files = Array.from(input.files ?? []);
    input.value = '';
    if (files.length > 0) onImportFiles(files);
  }

  registerCommands([
    {
      id: 'saveProject',
      title: 'Save project',
      group: 'Project',
      api: ['saveProjectContainer'],
      keywords: ['write', 'store'],
      enabled: () => dirty,
      run: () => onSave()
    },
    {
      id: 'newGroup',
      title: 'New group',
      group: 'Project',
      keywords: ['folder', 'collection', 'organize', 'library'],
      enabled: () => onCreateGroup !== undefined,
      run: () => onCreateGroup?.()
    },
    {
      id: 'exportProject',
      title: 'Export project',
      group: 'Project',
      api: ['saveProjectBundle', 'saveProjectContainer'],
      keywords: ['download', 'bundle', 'phxproj', 'archive', 'share', 'save as'],
      enabled: () => onExportProject !== undefined,
      run: () => {
        exportOpen = true;
      }
    },
    {
      id: 'projectDetails',
      title: 'Project details',
      group: 'Project',
      keywords: ['metadata', 'description', 'authors', 'tags', 'inspector'],
      enabled: () => onUpdateProjectMetadata !== undefined,
      run: () => {
        details = { scope: 'project' };
      }
    },
    {
      id: 'backToHome',
      title: 'Back to projects',
      group: 'Project',
      keywords: ['home', 'exit', 'close project'],
      run: () => onBack()
    }
  ]);
</script>

<div
  class="project"
  class:dragging
  data-testid="corpus"
  data-project-name={projectName}
  data-recording-count={visibleCount}
  data-saved-at={savedAt}
  role="region"
  aria-label={`Project ${projectName}`}
  ondragover={(event) => {
    event.preventDefault();
    dragging = true;
  }}
  ondragleave={(event) => {
    if (event.currentTarget === event.target) dragging = false;
  }}
  ondrop={handleDrop}
>
  <header class="top">
    <div class="left">
      <button type="button" class="ghost back" onclick={onBack} data-testid="back-home">
        <IconArrowLeft aria-hidden="true" />
        <span>Projects</span>
      </button>
      <span class="name">{projectName}</span>
    </div>
    <div class="right">
      <span class="dirty" data-testid="dirty-state" data-dirty={dirty}>
        {dirty ? 'Unsaved changes' : 'All changes saved'}
      </span>
      <button type="button" class="action" onclick={onSave} data-testid="save-project" disabled={!dirty}>
        <IconSave aria-hidden="true" />
        <span>Save</span>
      </button>
      {#if onStartRecording}
        <button
          type="button"
          class="record"
          data-testid="record"
          disabled={recording}
          onclick={() => onStartRecording?.()}
        >
          <IconMic aria-hidden="true" />
          <span>Record</span>
        </button>
      {/if}
      <button
        type="button"
        class="ghost icon-only"
        aria-label="Toggle theme"
        title={theme === 'light' ? 'Switch to dark' : 'Switch to light'}
        onclick={() => onThemeChange(theme === 'light' ? 'dark' : 'light')}
      >
        {#if theme === 'light'}
          <IconMoon aria-hidden="true" />
        {:else}
          <IconSun aria-hidden="true" />
        {/if}
      </button>
    </div>
  </header>

  <input
    bind:this={fileInput}
    type="file"
    accept=".wav,audio/wav,.aiff,.aif,audio/aiff,.flac,audio/flac,.TextGrid"
    multiple
    class="hidden-input"
    data-testid="corpus-file-input"
    onchange={handleInput}
  />

  <div class="workbench">
    <main class="body">
      {#if recordings.length === 0}
        <div class="empty" data-testid="corpus-empty">
          <p class="empty-lead">No recordings yet.</p>
          <p class="empty-sub">
            Drop WAV files here, or choose them. A TextGrid beside a WAV of the same name attaches as
            its annotation.
          </p>
          <div class="empty-actions">
            <button type="button" class="empty-action" data-testid="corpus-choose-files" onclick={() => fileInput?.click()}>
              <IconFolderOpen aria-hidden="true" />
              <span>Choose files</span>
            </button>
            {#if onStartRecording}
              <button
                type="button"
                class="empty-action record"
                data-testid="empty-record"
                disabled={recording}
                onclick={() => onStartRecording?.()}
              >
                <IconMic aria-hidden="true" />
                <span>Record</span>
              </button>
            {/if}
          </div>
        </div>
      {:else}
        <div class="toolbar">
          <div class="search">
            <IconSearch class="search-icon" aria-hidden="true" />
            <input
              type="search"
              class="search-input"
              data-testid="corpus-search"
              placeholder="Search recordings, tags, labels"
              autocomplete="off"
              spellcheck="false"
              bind:value={query}
            />
          </div>
          <div class="toolbar-actions">
            {#if onCreateGroup}
              <button type="button" class="tool" data-testid="new-group" onclick={() => onCreateGroup?.()}>
                <IconFolderPlus aria-hidden="true" />
                <span>New group</span>
              </button>
            {/if}
            {#if onExportProject}
              <button type="button" class="tool" data-testid="export-project" onclick={() => (exportOpen = true)}>
                <IconPackage aria-hidden="true" />
                <span>Export</span>
              </button>
            {/if}
            {#if onUpdateProjectMetadata}
              <button
                type="button"
                class="tool"
                class:active={details?.scope === 'project'}
                data-testid="project-details"
                onclick={() => (details = details?.scope === 'project' ? null : { scope: 'project' })}
              >
                <IconInfo aria-hidden="true" />
                <span>Details</span>
              </button>
            {/if}
          </div>
        </div>

        <LibraryTree
          {client}
          {theme}
          {recordings}
          {tree}
          collapsed={effectiveCollapsed}
          {hidden}
          {selectedMediaId}
          onOpen={(mediaId) => {
            const rec = byId.get(mediaId);
            if (rec) onOpenRecording(rec);
          }}
          onRenameRecording={(mediaId, name) => onRenameRecording?.(mediaId, name)}
          onDeleteRecording={(mediaId) => onDeleteRecording?.(mediaId)}
          onExportRecording={onExportRecording ? (mediaId) => onExportRecording?.(mediaId) : undefined}
          onShowDetails={showRecordingDetails}
          onToggleCollapse={(groupId) => onToggleCollapse?.(groupId)}
          onRenameGroup={(groupId, name) => onRenameGroup?.(groupId, name)}
          onDissolveGroup={(groupId) => onDissolveGroup?.(groupId)}
          onMove={(key, target, index) => onMoveNode?.(key, target, index)}
        />

        {#if visibleCount === 0 && query.trim()}
          <p class="no-hits" data-testid="corpus-search-empty">No recordings match “{query.trim()}”.</p>
        {/if}
      {/if}
    </main>

    {#if detailsSubject}
      {#key detailsSubject.key}
        <MetadataPanel
          scope={detailsSubject.scope}
          title={detailsSubject.title}
          description={detailsSubject.description}
          authors={detailsSubject.authors}
          tags={detailsSubject.tags}
          onSave={saveDetails}
          onClose={() => (details = null)}
        />
      {/key}
    {/if}
  </div>
</div>

{#if exportOpen && onExportProject}
  <ProjectExportDialog
    recordingCount={recordings.length}
    onExport={(mode) => {
      onExportProject?.(mode);
      exportOpen = false;
    }}
    onClose={() => (exportOpen = false)}
  />
{/if}

{#if removalUndo}
  <div class="undo-banner" role="status" data-testid="removal-undo">
    {#if removalUndo.stale}
      <span
        >Recording “{removalUndo.name}” removed. Another change happened since — restore it from the undo history
        (Ctrl+Z) instead.</span
      >
      <button type="button" class="undo" data-testid="removal-undo-action" disabled>
        <IconRotateCcw aria-hidden="true" />
        <span>Undo</span>
      </button>
    {:else}
      <span>Recording “{removalUndo.name}” removed.</span>
      <button type="button" class="undo" data-testid="removal-undo-action" onclick={() => onUndoRemoval?.()}>
        <IconRotateCcw aria-hidden="true" />
        <span>Undo</span>
      </button>
    {/if}
  </div>
{/if}

{#if dragging}
  <div class="drop-hint" aria-hidden="true"><span>Drop to add recordings</span></div>
{/if}

{#if busy}
  <div class="busy" role="status" data-testid="corpus-busy">{busyLabel}</div>
{/if}

<style>
  .project {
    position: relative;
    min-height: 100vh;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    background: var(--chrome);
    color: var(--text);
  }

  .project.dragging {
    outline: 2px dashed var(--accent);
    outline-offset: -8px;
  }

  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.7rem 1.1rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--panel);
  }

  .left,
  .right {
    display: flex;
    align-items: center;
    gap: 0.7rem;
  }

  .name {
    font-weight: 600;
  }

  button {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-md);
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.35rem 0.65rem;
    box-shadow: var(--shadow-sm);
    transition:
      background var(--t-fast),
      border-color var(--t-fast),
      color var(--t-fast);
  }

  button :global(svg) {
    font-size: 1rem;
    flex: none;
  }

  button:hover:not(:disabled) {
    background: var(--panel);
    border-color: color-mix(in oklab, var(--accent) 32%, var(--chrome-strong));
  }

  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    box-shadow: none;
  }

  .ghost {
    color: var(--muted);
    background: transparent;
    box-shadow: none;
  }

  .ghost:hover:not(:disabled) {
    color: var(--text);
  }

  .icon-only {
    padding: 0.35rem;
    width: 2.15rem;
    justify-content: center;
  }

  .action {
    border-color: var(--action);
    background: var(--action);
    color: #fff;
  }

  .action:hover:not(:disabled) {
    background: var(--action-strong);
    border-color: var(--action-strong);
  }

  .action:disabled {
    background: var(--panel-soft);
    border-color: var(--chrome-strong);
    color: var(--muted);
  }

  .record :global(svg) {
    color: var(--danger);
  }

  .dirty {
    font-size: 0.8rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .dirty[data-dirty='true'] {
    color: var(--warn);
  }

  .workbench {
    min-height: 0;
    display: flex;
    overflow: hidden;
  }

  .body {
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: auto;
    padding: 1.25rem 1.5rem;
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    margin-bottom: 0.9rem;
    flex-wrap: wrap;
  }

  .search {
    position: relative;
    display: flex;
    align-items: center;
    flex: 1;
    max-width: 24rem;
  }

  .search :global(.search-icon) {
    position: absolute;
    left: 0.6rem;
    color: var(--muted);
    font-size: 0.95rem;
    pointer-events: none;
  }

  .search-input {
    width: 100%;
    font: inherit;
    color: var(--text);
    background: var(--panel);
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-md);
    padding: 0.4rem 0.6rem 0.4rem 2rem;
  }

  .search-input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--accent) 22%, transparent);
  }

  .toolbar-actions {
    display: flex;
    gap: 0.5rem;
  }

  .tool {
    color: var(--muted);
  }

  .tool.active {
    color: var(--accent-strong);
    border-color: color-mix(in oklab, var(--accent) 45%, var(--chrome-strong));
    background: var(--accent-tint);
  }

  .no-hits {
    margin-top: 1rem;
    color: var(--muted);
    font-size: 0.9rem;
  }

  .empty {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    text-align: center;
    color: var(--muted);
  }

  .empty-lead {
    margin: 0;
    font-size: 1.1rem;
    color: var(--text);
  }

  .empty-sub {
    margin: 0;
    max-width: 32rem;
    font-size: 0.9rem;
  }

  .empty-actions {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.4rem;
  }

  .empty-action.record :global(svg) {
    color: var(--danger);
  }

  .record:disabled {
    opacity: 0.5;
  }

  .hidden-input {
    display: none;
  }

  .undo-banner {
    position: fixed;
    left: 50%;
    bottom: 1.25rem;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 0.9rem;
    padding: 0.5rem 0.6rem 0.5rem 0.95rem;
    border-radius: var(--radius-lg);
    border: 1px solid var(--chrome-strong);
    background: var(--panel);
    color: var(--text);
    box-shadow: var(--shadow-lg);
    font-size: 0.85rem;
    z-index: 15;
  }

  .undo-banner .undo {
    color: var(--accent-strong);
    background: transparent;
    box-shadow: none;
    border-color: transparent;
  }

  .undo-banner .undo:hover {
    background: var(--accent-tint);
    border-color: color-mix(in oklab, var(--accent) 30%, transparent);
  }

  .undo-banner .undo:disabled {
    color: var(--muted);
    cursor: default;
  }

  .undo-banner .undo:disabled:hover {
    background: transparent;
    border-color: transparent;
  }

  .drop-hint {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: color-mix(in oklab, var(--accent) 12%, transparent);
    pointer-events: none;
  }

  .drop-hint span {
    padding: 0.6rem 1.1rem;
    border-radius: 8px;
    background: var(--panel);
    border: 1px solid var(--accent);
    color: var(--text);
    font-weight: 600;
  }

  .busy {
    position: fixed;
    left: 50%;
    bottom: 1.25rem;
    transform: translateX(-50%);
    padding: 0.55rem 0.95rem;
    border-radius: var(--radius-lg);
    border: 1px solid var(--chrome-strong);
    background: var(--panel);
    color: var(--text);
    box-shadow: var(--shadow-lg);
    font-size: 0.85rem;
  }
</style>
