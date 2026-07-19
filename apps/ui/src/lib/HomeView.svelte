<script lang="ts">
  import IconPlus from '~icons/lucide/plus';
  import IconFolderOpen from '~icons/lucide/folder-open';
  import IconPackageOpen from '~icons/lucide/package-open';
  import IconMic from '~icons/lucide/mic';
  import IconSun from '~icons/lucide/sun';
  import IconMoon from '~icons/lucide/moon';
  import IconSparkles from '~icons/lucide/sparkles';
  import IconSearch from '~icons/lucide/search';
  import IconFolderPlus from '~icons/lucide/folder-plus';
  import IconChevronRight from '~icons/lucide/chevron-right';
  import IconFolder from '~icons/lucide/folder';
  import IconUngroup from '~icons/lucide/ungroup';
  import IconDownload from '~icons/lucide/download';
  import IconTrash from '~icons/lucide/trash-2';
  import ProjectCard from './ProjectCard.svelte';
  import InlineRename from './InlineRename.svelte';
  import { filesFromDataTransfer } from './dnd';
  import { registerCommands } from './commands.svelte';
  import type { HomeIndex, ProjectSummary } from './types';

  interface Props {
    projects: ProjectSummary[];
    busy: boolean;
    busyLabel: string;
    theme: 'light' | 'dark';
    onImportFiles: (files: File[]) => void;
    onNewProject: (name: string) => void;
    onOpenSample?: () => void;
    /** Opens an uploaded `.phxproj` file; absent hides the open-file action. */
    onOpenProjectFile?: (file: File) => void;
    onOpenProject: (id: string) => void;
    onRenameProject: (id: string, name: string) => void;
    onDeleteProject: (id: string) => void;
    onDuplicateProject: (id: string) => void;
    onThemeChange: (theme: 'light' | 'dark') => void;
    /** Starts a microphone recording; absent when the browser cannot capture. */
    onStartRecording?: () => void;
    /** Whether a take is currently being captured. */
    recording?: boolean;

    // --- Home management (the web shell wires these; when the index is absent
    // the grid falls back to a flat, unmanaged listing). ---

    /** Pins and project groups; a flat grid when omitted. */
    homeIndex?: HomeIndex;
    onTogglePin?: (id: string) => void;
    /** Creates a group, seeded with `memberIds` (empty for a bare group). */
    onCreateGroupFrom?: (memberIds: string[]) => void;
    onRenameGroup?: (groupId: string, name: string) => void;
    onDissolveGroup?: (groupId: string) => void;
    onToggleGroupCollapse?: (groupId: string) => void;
    /** Moves a project into `groupId`, or out of every group when null. */
    onMoveToGroup?: (id: string, groupId: string | null) => void;
    /** Exports one stored project as a self-contained bundle download. */
    onExportStored?: (id: string) => Promise<void>;
    onBatchDelete?: (ids: string[]) => void;
  }

  let {
    projects,
    busy,
    busyLabel,
    theme,
    onImportFiles,
    onNewProject,
    onOpenSample,
    onOpenProjectFile,
    onOpenProject,
    onRenameProject,
    onDeleteProject,
    onDuplicateProject,
    onThemeChange,
    onStartRecording,
    recording = false,
    homeIndex,
    onTogglePin,
    onCreateGroupFrom,
    onRenameGroup,
    onDissolveGroup,
    onToggleGroupCollapse,
    onMoveToGroup,
    onExportStored,
    onBatchDelete
  }: Props = $props();

  let dragging = $state(false);
  let newName = $state('');
  let fileInput = $state<HTMLInputElement | null>(null);
  let projectInput = $state<HTMLInputElement | null>(null);

  const manage = $derived(homeIndex !== undefined);
  const index = $derived(homeIndex ?? { pinned: [], groups: [] });
  const byId = $derived(new Map(projects.map((p) => [p.id, p])));

  let query = $state('');
  const q = $derived(query.trim().toLowerCase());

  function matchesQuery(p: ProjectSummary): boolean {
    if (!q) return true;
    if (p.name.toLowerCase().includes(q)) return true;
    return (p.tags ?? []).some((tag) => tag.toLowerCase().includes(q));
  }

  const pinnedSet = $derived(new Set(index.pinned));

  const pinnedProjects = $derived(
    index.pinned
      .map((id) => byId.get(id))
      .filter((p): p is ProjectSummary => p !== undefined)
      .filter(matchesQuery)
  );

  const groupedIds = $derived(new Set(index.groups.flatMap((g) => g.members)));

  const groupSections = $derived(
    index.groups.map((g) => {
      const members = g.members
        .map((id) => byId.get(id))
        .filter((p): p is ProjectSummary => p !== undefined && !pinnedSet.has(p.id));
      return {
        id: g.id,
        name: g.name,
        collapsed: g.collapsed,
        count: members.length,
        visibleMembers: members.filter(matchesQuery)
      };
    })
  );

  const ungrouped = $derived(
    projects
      .filter((p) => !pinnedSet.has(p.id) && !groupedIds.has(p.id))
      .filter(matchesQuery)
  );

  // Flattened id order across every visible section, so a Shift-range selects a
  // contiguous run the way it reads on screen. A collapsed group hides its
  // members here (unless a search forces them open), matching what is shown.
  const flatVisible = $derived([
    ...pinnedProjects.map((p) => p.id),
    ...groupSections.flatMap((g) => (q || !g.collapsed ? g.visibleMembers.map((p) => p.id) : [])),
    ...ungrouped.map((p) => p.id)
  ]);

  // --- Multi-selection ---

  let selected = $state(new Set<string>());
  let anchor = $state<string | null>(null);

  const selectedOrdered = $derived(flatVisible.filter((id) => selected.has(id)));

  function toggleSelect(id: string) {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selected = next;
  }

  function selectRange(from: string, to: string) {
    const order = flatVisible;
    const a = order.indexOf(from);
    const b = order.indexOf(to);
    if (a < 0 || b < 0) {
      toggleSelect(to);
      anchor = to;
      return;
    }
    const [lo, hi] = a <= b ? [a, b] : [b, a];
    const next = new Set(selected);
    for (let i = lo; i <= hi; i += 1) next.add(order[i]);
    selected = next;
    anchor = to;
  }

  function clearSelection() {
    selected = new Set();
    anchor = null;
  }

  function activate(id: string, event: MouseEvent | KeyboardEvent) {
    const mod = event instanceof MouseEvent && (event.metaKey || event.ctrlKey);
    if (manage && mod) {
      toggleSelect(id);
      anchor = id;
      return;
    }
    if (manage && event.shiftKey && anchor) {
      selectRange(anchor, id);
      return;
    }
    clearSelection();
    onOpenProject(id);
  }

  // --- Batch operations ---

  let exportProgress = $state<{ done: number; total: number } | null>(null);
  let confirmingDelete = $state(false);

  async function batchExport() {
    if (!onExportStored) return;
    const ids = [...selectedOrdered];
    if (ids.length === 0) return;
    exportProgress = { done: 0, total: ids.length };
    for (const id of ids) {
      await onExportStored(id);
      exportProgress = { done: (exportProgress?.done ?? 0) + 1, total: ids.length };
    }
    exportProgress = null;
    clearSelection();
  }

  function groupSelection() {
    onCreateGroupFrom?.([...selectedOrdered]);
    clearSelection();
  }

  function confirmDelete() {
    onBatchDelete?.([...selectedOrdered]);
    confirmingDelete = false;
    clearSelection();
  }

  // --- Drag a card between groups ---

  let dragId = $state<string | null>(null);
  let dropTarget = $state<string | null>(null);

  function beginCardDrag(id: string, event: PointerEvent) {
    event.preventDefault();
    dragId = id;
  }

  function zoneUnderPoint(x: number, y: number): string | null {
    const el = document.elementFromPoint(x, y)?.closest<HTMLElement>('[data-drop-zone]');
    return el?.dataset.dropZone ?? null;
  }

  function handlePointerMove(event: PointerEvent) {
    if (!dragId) return;
    dropTarget = zoneUnderPoint(event.clientX, event.clientY);
  }

  function handlePointerUp() {
    if (dragId && dropTarget !== null) {
      onMoveToGroup?.(dragId, dropTarget === '__ungrouped__' ? null : dropTarget);
    }
    dragId = null;
    dropTarget = null;
  }

  const PALETTE_HINT_KEY = 'phonix-hint-palette';
  const isMac =
    typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
  const paletteKey = isMac ? '⌘K' : 'Ctrl-K';

  let paletteHintDismissed = $state(
    typeof localStorage !== 'undefined' && localStorage.getItem(PALETTE_HINT_KEY) === 'dismissed'
  );

  function dismissPaletteHint() {
    paletteHintDismissed = true;
    try {
      localStorage.setItem(PALETTE_HINT_KEY, 'dismissed');
    } catch {
      // A blocked storage only means the hint returns next visit; not worth surfacing.
    }
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

  function createProject() {
    onNewProject(newName);
    newName = '';
  }

  function handleProjectInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (file) onOpenProjectFile?.(file);
  }

  registerCommands([
    {
      id: 'importAudioFiles',
      title: 'Import audio files',
      group: 'Project',
      keywords: ['open', 'add recordings', 'wav', 'choose files'],
      run: () => fileInput?.click()
    },
    {
      id: 'openProjectFile',
      title: 'Open project file',
      group: 'Project',
      api: ['readProjectBundle'],
      keywords: ['import', 'phxproj', 'bundle', 'load project', 'upload'],
      enabled: () => onOpenProjectFile !== undefined,
      run: () => projectInput?.click()
    },
    {
      id: 'openSampleProject',
      title: 'Open sample project',
      group: 'Project',
      keywords: ['demo', 'example', 'arctic', 'try'],
      enabled: () => onOpenSample !== undefined,
      run: () => onOpenSample?.()
    },
    {
      id: 'newProjectGroup',
      title: 'New project group',
      group: 'Project',
      keywords: ['collection', 'folder', 'organize', 'home'],
      enabled: () => onCreateGroupFrom !== undefined,
      run: () => onCreateGroupFrom?.([])
    }
  ]);
</script>

{#snippet card(p: ProjectSummary)}
  <ProjectCard
    project={p}
    pinned={pinnedSet.has(p.id)}
    selected={selected.has(p.id)}
    dragging={dragId === p.id}
    onActivate={activate}
    onRename={onRenameProject}
    onDelete={onDeleteProject}
    onDuplicate={onDuplicateProject}
    onTogglePin={manage ? onTogglePin : undefined}
    onDragStart={manage && onMoveToGroup ? beginCardDrag : undefined}
  />
{/snippet}

{#snippet addCard()}
  <button
    type="button"
    class="add-card"
    data-testid="new-project-card"
    aria-label="New project"
    onclick={() => onNewProject('')}
  >
    <IconPlus aria-hidden="true" />
    <span>New project</span>
  </button>
{/snippet}

<svelte:window onpointermove={handlePointerMove} onpointerup={handlePointerUp} />

<div
  class="home"
  class:dragging
  data-testid="home"
  role="region"
  aria-label="Projects"
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
    <div class="tools">
      <form
        class="create"
        onsubmit={(event) => {
          event.preventDefault();
          createProject();
        }}
      >
        <input
          type="text"
          placeholder="New project name"
          bind:value={newName}
          data-testid="new-project-name"
          aria-label="New project name"
        />
        <button type="submit" class="action" data-testid="new-project">
          <IconPlus aria-hidden="true" />
          <span>Create</span>
        </button>
      </form>
      <button type="button" class="ghost" onclick={() => fileInput?.click()} data-testid="choose-files">
        <IconFolderOpen aria-hidden="true" />
        <span>Choose files</span>
      </button>
      {#if onOpenProjectFile}
        <button type="button" class="ghost" onclick={() => projectInput?.click()} data-testid="open-project-file">
          <IconPackageOpen aria-hidden="true" />
          <span>Open project file</span>
        </button>
      {/if}
      {#if onStartRecording}
        <button
          type="button"
          class="ghost record"
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
    data-testid="folder-input"
    onchange={handleInput}
  />

  <input
    bind:this={projectInput}
    type="file"
    accept=".phxproj,application/zip"
    class="hidden-input"
    data-testid="project-file-input"
    onchange={handleProjectInput}
  />

  <main class="body">
    {#if projects.length === 0}
      <div class="empty" data-testid="home-empty">
        <p class="lead">
          Phonia analyzes recorded speech — waveform, spectrogram, pitch, and annotation tiers in
          one workspace.
        </p>
        <p class="sub">Drop a folder of recordings to start, or use the buttons below.</p>
        <div class="empty-actions">
          {#if onOpenSample}
            <button type="button" class="primary" data-testid="open-sample" onclick={onOpenSample}>
              <IconSparkles aria-hidden="true" />
              <span>Open sample project</span>
            </button>
          {/if}
          <button type="button" class="ghost" data-testid="empty-choose-files" onclick={() => fileInput?.click()}>
            <IconFolderOpen aria-hidden="true" />
            <span>Choose files</span>
          </button>
          {#if onStartRecording}
            <button
              type="button"
              class="ghost record"
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
    {:else if !manage}
      <div class="grid" data-testid="project-grid">
        {#each projects as project (project.id)}
          {@render card(project)}
        {/each}
        {@render addCard()}
      </div>
    {:else}
      <div class="toolbar">
        <div class="search">
          <IconSearch class="search-icon" aria-hidden="true" />
          <input
            type="search"
            class="search-input"
            data-testid="home-search"
            placeholder="Search projects, tags"
            autocomplete="off"
            spellcheck="false"
            bind:value={query}
          />
        </div>
        <button type="button" class="tool" data-testid="home-new-group" onclick={() => onCreateGroupFrom?.([])}>
          <IconFolderPlus aria-hidden="true" />
          <span>New group</span>
        </button>
      </div>

      {#if selectedOrdered.length > 0}
        <div class="seltoolbar" role="toolbar" aria-label="Selection actions" data-testid="selection-toolbar">
          <span class="selcount" data-testid="selection-count">{selectedOrdered.length} selected</span>
          <div class="selactions">
            {#if onExportStored}
              <button type="button" data-testid="batch-export" disabled={exportProgress !== null} onclick={batchExport}>
                <IconDownload aria-hidden="true" /><span>Export</span>
              </button>
            {/if}
            {#if onCreateGroupFrom}
              <button type="button" data-testid="batch-group" onclick={groupSelection}>
                <IconFolderPlus aria-hidden="true" /><span>Group</span>
              </button>
            {/if}
            {#if onBatchDelete}
              <button type="button" class="danger" data-testid="batch-delete" onclick={() => (confirmingDelete = true)}>
                <IconTrash aria-hidden="true" /><span>Delete</span>
              </button>
            {/if}
            <button type="button" class="ghost" data-testid="selection-clear" onclick={clearSelection}>Clear</button>
          </div>
        </div>
      {/if}

      {#if pinnedProjects.length > 0}
        <section class="section" data-testid="home-pinned" aria-label="Pinned projects">
          <div class="grid">
            {#each pinnedProjects as project (project.id)}
              {@render card(project)}
            {/each}
          </div>
        </section>
      {/if}

      {#each groupSections as group (group.id)}
        <section
          class="section group"
          class:drop={dropTarget === group.id}
          data-testid="home-group"
          data-group-id={group.id}
          data-drop-zone={group.id}
        >
          <header class="section-head group-head">
            <button
              type="button"
              class="disclose"
              data-testid="group-disclose"
              aria-label={group.collapsed ? 'Expand group' : 'Collapse group'}
              aria-expanded={!group.collapsed}
              onclick={() => onToggleGroupCollapse?.(group.id)}
            >
              <span class="chev" class:open={!group.collapsed}><IconChevronRight aria-hidden="true" /></span>
            </button>
            <IconFolder class="section-icon" aria-hidden="true" />
            <InlineRename
              name={group.name}
              class="group-name"
              label="Rename group"
              testId="rename-group"
              onRename={(next) => onRenameGroup?.(group.id, next)}
            />
            <span class="count" data-testid="group-count">{group.count}</span>
            <button
              type="button"
              class="dissolve"
              aria-label="Dissolve group"
              title="Dissolve group"
              data-testid="dissolve-group"
              onclick={() => onDissolveGroup?.(group.id)}
            >
              <IconUngroup aria-hidden="true" />
            </button>
          </header>
          {#if !group.collapsed || q}
            {#if group.visibleMembers.length > 0}
              <div class="grid">
                {#each group.visibleMembers as project (project.id)}
                  {@render card(project)}
                {/each}
              </div>
            {:else}
              <p class="zone-hint">Drag projects here.</p>
            {/if}
          {/if}
        </section>
      {/each}

      <section
        class="section"
        class:divider={index.groups.length > 0}
        class:drop={dropTarget === '__ungrouped__'}
        data-testid="home-ungrouped"
        data-drop-zone="__ungrouped__"
        aria-label={index.groups.length > 0 ? 'Other projects' : undefined}
      >
        {#if ungrouped.length === 0 && index.groups.length > 0}
          <p class="zone-hint">Drag projects here to ungroup them.</p>
        {/if}
        <div class="grid">
          {#each ungrouped as project (project.id)}
            {@render card(project)}
          {/each}
          {@render addCard()}
        </div>
      </section>

      {#if q && flatVisible.length === 0}
        <p class="no-hits" data-testid="home-search-empty">No projects match “{query.trim()}”.</p>
      {/if}
    {/if}
  </main>

  {#if exportProgress}
    <div class="export-progress" role="status" data-testid="export-progress">
      Exporting {exportProgress.done} of {exportProgress.total}…
    </div>
  {/if}

  {#if dragging}
    <div class="drop-hint" data-testid="drop-hint" aria-hidden="true">
      <span>Drop to import</span>
    </div>
  {/if}

  {#if busy}
    <div class="busy" role="status" data-testid="home-busy">{busyLabel}</div>
  {/if}

  {#if !paletteHintDismissed}
    <div class="palette-hint" data-testid="palette-hint">
      <span>Press <kbd>{paletteKey}</kbd> to open the command palette and search every action by name.</span>
      <button type="button" class="hint-close" aria-label="Dismiss hint" onclick={dismissPaletteHint}>×</button>
    </div>
  {/if}
</div>

{#if confirmingDelete}
  <div class="modal-backdrop" data-testid="batch-delete-confirm">
    <div class="modal" role="dialog" aria-modal="true" aria-label="Delete projects">
      <h2>
        Delete {selectedOrdered.length}
        {selectedOrdered.length === 1 ? 'project' : 'projects'}?
      </h2>
      <p>
        This permanently removes {selectedOrdered.length === 1 ? 'it' : 'them'} and every recording
        inside. It cannot be undone.
      </p>
      <div class="modal-actions">
        <button
          type="button"
          class="secondary"
          data-testid="batch-delete-cancel"
          onclick={() => (confirmingDelete = false)}
        >
          Cancel
        </button>
        <button type="button" class="destructive" data-testid="batch-delete-confirm-action" onclick={confirmDelete}>
          Delete
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .home {
    position: relative;
    min-height: 100vh;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    background: var(--chrome);
    color: var(--text);
  }

  .home.dragging {
    outline: 2px dashed var(--accent);
    outline-offset: -8px;
  }

  .top {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 1rem;
    padding: 0.85rem 1.25rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--panel);
  }

  .tools {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    flex-wrap: wrap;
    gap: 0.5rem;
    min-width: 0;
  }

  .create {
    display: flex;
    gap: 0.4rem;
    min-width: 0;
  }

  .create input {
    min-width: 8rem;
    flex: 1 1 12rem;
    padding: 0.4rem 0.65rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-md);
    background: var(--panel-soft);
    color: var(--text);
    transition:
      border-color var(--t-fast),
      box-shadow var(--t-fast);
  }

  .create input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--accent) 22%, transparent);
  }

  button {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    flex: none;
    white-space: nowrap;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-md);
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.4rem 0.7rem;
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
  }

  .icon-only {
    padding: 0.4rem;
    width: 2.25rem;
    justify-content: center;
  }

  /* Primary form actions use the teal identity accent. */
  .action {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--on-accent);
  }

  .action:hover:not(:disabled) {
    background: var(--accent-strong);
    border-color: var(--accent-strong);
  }

  .ghost {
    color: var(--muted);
    box-shadow: none;
    background: transparent;
  }

  .ghost:hover:not(:disabled) {
    color: var(--text);
  }

  .record {
    color: var(--text);
  }

  .record :global(svg) {
    color: var(--danger);
  }

  .record:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .hidden-input {
    display: none;
  }

  .body {
    min-height: 0;
    overflow: auto;
    padding: 1.5rem;
  }

  .empty {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.6rem;
    text-align: center;
    color: var(--muted);
  }

  .empty .lead {
    font-size: 1.15rem;
    color: var(--text);
  }

  .empty .sub {
    max-width: 34rem;
    font-size: 0.9rem;
  }

  .empty-actions {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.4rem;
  }

  .empty-actions .primary {
    border-color: var(--accent);
    background: var(--accent-tint);
    color: var(--accent-strong);
    font-weight: 500;
  }

  .empty-actions .primary:hover {
    background: color-mix(in oklab, var(--accent) 24%, var(--panel));
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    margin-bottom: 1rem;
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
    box-shadow: none;
  }

  .search-input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--accent) 22%, transparent);
  }

  .tool {
    color: var(--muted);
  }

  .seltoolbar {
    position: sticky;
    top: 0;
    z-index: 2;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    margin-bottom: 1rem;
    padding: 0.5rem 0.75rem;
    border: 1px solid color-mix(in oklab, var(--accent) 40%, var(--chrome-strong));
    border-radius: var(--radius-lg);
    background: var(--accent-tint);
    box-shadow: var(--shadow-sm);
  }

  .selcount {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--accent-strong);
    font-variant-numeric: tabular-nums;
  }

  .selactions {
    display: flex;
    gap: 0.4rem;
  }

  .selactions button {
    padding: 0.35rem 0.6rem;
    font-size: 0.8rem;
  }

  .selactions .danger {
    color: var(--danger);
  }

  .selactions .danger:hover:not(:disabled) {
    border-color: color-mix(in oklab, var(--danger) 45%, transparent);
    color: var(--danger);
  }

  .section {
    margin-bottom: 1.5rem;
  }

  .section.group,
  .section.divider {
    padding-top: 0.85rem;
    border-top: 1px solid var(--chrome-strong);
  }

  .section.drop {
    outline: 2px dashed var(--accent);
    outline-offset: 4px;
    background: color-mix(in oklab, var(--accent) 8%, transparent);
  }

  .section-head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.75rem;
    font-size: 0.72rem;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted);
    font-weight: 600;
  }

  .group-head {
    text-transform: none;
    letter-spacing: normal;
    font-size: 0.95rem;
    color: var(--text);
  }

  .section-head :global(.section-icon) {
    font-size: 0.95rem;
    color: var(--accent-strong);
  }

  .count {
    font-size: 0.72rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    padding: 0.05rem 0.4rem;
    border-radius: 999px;
    background: var(--panel);
    border: 1px solid var(--chrome-strong);
  }

  .disclose {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    color: var(--muted);
    padding: 0.1rem;
    box-shadow: none;
  }

  .disclose:hover:not(:disabled) {
    background: transparent;
    color: var(--text);
  }

  .chev {
    display: inline-flex;
    transition: transform var(--t-fast);
  }

  .chev.open {
    transform: rotate(90deg);
  }

  :global(.group-name) {
    font-weight: 600;
  }

  .dissolve {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid transparent;
    background: transparent;
    color: var(--muted);
    padding: 0.22rem;
    box-shadow: none;
  }

  .dissolve:hover:not(:disabled) {
    color: var(--text);
    background: var(--panel);
    border-color: var(--chrome-strong);
  }

  .zone-hint {
    margin: 0.25rem 0.25rem 0.5rem;
    font-size: 0.82rem;
    color: var(--muted);
  }

  .no-hits {
    margin-top: 0.5rem;
    color: var(--muted);
    font-size: 0.9rem;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
    gap: 1rem;
  }

  .add-card {
    min-height: calc(6.5rem + 0.4rem + 0.4rem + 0.2rem + 0.2rem + 0.75rem + 0.375rem + 5px);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    padding: 1rem;
    border: 1px dashed var(--chrome-strong);
    border-radius: var(--radius-xl);
    background: transparent;
    color: var(--muted);
    box-shadow: none;
  }

  .add-card :global(svg) {
    font-size: 1.4rem;
  }

  .add-card span {
    font-size: 0.85rem;
  }

  .add-card:hover:not(:disabled),
  .add-card:focus-visible {
    border-color: color-mix(in oklab, var(--accent) 32%, var(--chrome-strong));
    background: transparent;
    color: var(--text);
  }

  .export-progress {
    position: fixed;
    left: 50%;
    bottom: 1.25rem;
    transform: translateX(-50%);
    padding: 0.55rem 0.95rem;
    border-radius: var(--radius-lg);
    border: 1px solid color-mix(in oklab, var(--accent) 40%, var(--chrome-strong));
    background: var(--panel);
    color: var(--text);
    box-shadow: var(--shadow-lg);
    font-size: 0.85rem;
    font-variant-numeric: tabular-nums;
    z-index: 16;
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

  .palette-hint {
    position: absolute;
    left: 1.25rem;
    bottom: 1.25rem;
    display: flex;
    align-items: center;
    gap: 0.6rem;
    max-width: min(30rem, calc(100vw - 2.5rem));
    padding: 0.55rem 0.7rem 0.55rem 0.9rem;
    border-radius: var(--radius-lg);
    border: 1px solid var(--chrome-strong);
    background: var(--panel);
    color: var(--muted);
    box-shadow: var(--shadow-md);
    font-size: 0.82rem;
  }

  .palette-hint kbd {
    border: 1px solid var(--chrome-strong);
    border-radius: 4px;
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.05rem 0.36rem;
    font-family: var(--font-mono);
    font-size: 0.76rem;
    font-variant-numeric: tabular-nums;
  }

  .hint-close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 1.05rem;
    line-height: 1;
    padding: 0 0.2rem;
    box-shadow: none;
  }

  .hint-close:hover:not(:disabled) {
    background: transparent;
    border: none;
  }

  .hint-close:hover {
    color: var(--text);
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: color-mix(in oklab, #000 52%, transparent);
    backdrop-filter: blur(2px);
    z-index: 30;
  }

  .modal {
    max-width: 26rem;
    padding: 1.25rem 1.4rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-xl);
    background: var(--panel);
    color: var(--text);
    box-shadow: var(--shadow-lg);
  }

  .modal h2 {
    margin: 0 0 0.5rem;
    font-size: 1.05rem;
  }

  .modal p {
    margin: 0 0 1rem;
    color: var(--muted);
    font-size: 0.9rem;
    line-height: 1.45;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  .modal-actions button {
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-md);
    padding: 0.45rem 0.95rem;
    background: var(--panel-soft);
    color: var(--text);
  }

  .modal-actions button:hover:not(:disabled) {
    background: var(--panel);
    border-color: color-mix(in oklab, var(--accent) 32%, var(--chrome-strong));
  }

  .modal-actions .destructive {
    border-color: var(--danger);
    background: var(--danger);
    color: #fff;
  }

  .modal-actions .destructive:hover:not(:disabled) {
    background: color-mix(in oklab, var(--danger) 85%, #000);
    border-color: color-mix(in oklab, var(--danger) 85%, #000);
  }
</style>
