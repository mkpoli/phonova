<script lang="ts">
  import IconPlus from '~icons/lucide/plus';
  import IconFolderOpen from '~icons/lucide/folder-open';
  import IconPackageOpen from '~icons/lucide/package-open';
  import IconMic from '~icons/lucide/mic';
  import IconSun from '~icons/lucide/sun';
  import IconMoon from '~icons/lucide/moon';
  import IconSparkles from '~icons/lucide/sparkles';
  import ProjectCard from './ProjectCard.svelte';
  import { filesFromDataTransfer } from './dnd';
  import { registerCommands } from './commands.svelte';
  import type { ProjectSummary } from './types';

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
    recording = false
  }: Props = $props();

  let dragging = $state(false);
  let newName = $state('');
  let fileInput = $state<HTMLInputElement | null>(null);
  let projectInput = $state<HTMLInputElement | null>(null);

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
    }
  ]);
</script>

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
    <div class="brand">
      <span class="mark" aria-hidden="true"></span>
      <span class="title">Phonia</span>
    </div>
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
    accept=".wav,audio/wav,.TextGrid"
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
        <p class="lead">Drop a folder of recordings here to start a project.</p>
        <p class="sub">
          Every WAV becomes a browsable entry. A TextGrid beside a WAV of the same name attaches as
          its annotation.
        </p>
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
        {#if onOpenSample}
          <p class="empty-note">
            The sample holds two CMU ARCTIC sentences with word tiers and a synthesized vowel.
          </p>
        {/if}
      </div>
    {:else}
      <div class="grid" data-testid="project-grid">
        {#each projects as project (project.id)}
          <ProjectCard
            {project}
            onOpen={onOpenProject}
            onRename={onRenameProject}
            onDelete={onDeleteProject}
            onDuplicate={onDuplicateProject}
          />
        {/each}
      </div>
    {/if}
  </main>

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
    justify-content: space-between;
    gap: 1rem;
    padding: 0.85rem 1.25rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--panel);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.55rem;
  }

  .mark {
    width: 1rem;
    height: 1rem;
    border-radius: 4px;
    background: linear-gradient(140deg, var(--accent), var(--accent-strong));
    box-shadow: 0 0 0 1px color-mix(in oklab, var(--accent) 30%, transparent);
  }

  .title {
    font-family: var(--font-serif);
    font-size: 1.35rem;
    font-weight: 600;
    letter-spacing: 0.01em;
    color: var(--accent-strong);
  }

  .tools {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .create {
    display: flex;
    gap: 0.4rem;
  }

  .create input {
    min-width: 12rem;
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

  .icon-only {
    padding: 0.4rem;
    width: 2.25rem;
    justify-content: center;
  }

  /* The one primary form action is blue, kept distinct from the teal brand. */
  .action {
    border-color: var(--action);
    background: var(--action);
    color: #fff;
  }

  .action:hover:not(:disabled) {
    background: var(--action-strong);
    border-color: var(--action-strong);
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

  .empty-note {
    font-size: 0.82rem;
    color: var(--muted);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
    gap: 1rem;
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
    position: fixed;
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
</style>
