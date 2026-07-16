<script lang="ts">
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
    onOpenProject: (id: string) => void;
    onRenameProject: (id: string, currentName: string) => void;
    onDeleteProject: (id: string) => void;
    onDuplicateProject: (id: string) => void;
    onThemeChange: (theme: 'light' | 'dark') => void;
  }

  let {
    projects,
    busy,
    busyLabel,
    theme,
    onImportFiles,
    onNewProject,
    onOpenSample,
    onOpenProject,
    onRenameProject,
    onDeleteProject,
    onDuplicateProject,
    onThemeChange
  }: Props = $props();

  let dragging = $state(false);
  let newName = $state('');
  let fileInput = $state<HTMLInputElement | null>(null);

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

  registerCommands([
    {
      id: 'importAudioFiles',
      title: 'Import audio files',
      group: 'Project',
      keywords: ['open', 'add recordings', 'wav', 'choose files'],
      run: () => fileInput?.click()
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
      <span class="title">Phonix</span>
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
        <button type="submit" data-testid="new-project">Create</button>
      </form>
      <button type="button" class="ghost" onclick={() => fileInput?.click()} data-testid="choose-files">
        Choose files
      </button>
      <button
        type="button"
        class="ghost"
        aria-label="Toggle theme"
        onclick={() => onThemeChange(theme === 'light' ? 'dark' : 'light')}
      >
        {theme === 'light' ? 'Dark' : 'Light'}
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
              Open sample project
            </button>
          {/if}
          <button type="button" class="ghost" data-testid="empty-choose-files" onclick={() => fileInput?.click()}>
            Choose files
          </button>
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
    width: 0.9rem;
    height: 0.9rem;
    border-radius: 3px;
    background: linear-gradient(140deg, var(--accent), var(--accent-strong));
  }

  .title {
    font-weight: 600;
    letter-spacing: 0.01em;
  }

  .tools {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .create {
    display: flex;
    gap: 0.35rem;
  }

  .create input {
    min-width: 12rem;
    padding: 0.35rem 0.6rem;
    border: 1px solid var(--chrome-strong);
    border-radius: 6px;
    background: var(--panel-soft);
    color: var(--text);
  }

  button {
    border: 1px solid var(--chrome-strong);
    border-radius: 6px;
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.35rem 0.7rem;
  }

  button:hover {
    background: var(--panel);
  }

  .ghost {
    color: var(--muted);
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
    background: color-mix(in oklab, var(--accent) 20%, var(--panel-soft));
    color: var(--text);
  }

  .empty-note {
    font-size: 0.82rem;
    color: var(--muted);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(15rem, 1fr));
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
    padding: 0.5rem 0.9rem;
    border-radius: 8px;
    border: 1px solid var(--chrome-strong);
    background: var(--panel);
    color: var(--text);
    box-shadow: 0 10px 30px rgba(15, 23, 42, 0.18);
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
    padding: 0.45rem 0.6rem 0.45rem 0.85rem;
    border-radius: 8px;
    border: 1px solid var(--chrome-strong);
    background: var(--panel);
    color: var(--muted);
    box-shadow: 0 10px 30px rgba(15, 23, 42, 0.14);
    font-size: 0.82rem;
  }

  .palette-hint kbd {
    border: 1px solid var(--chrome-strong);
    border-radius: 4px;
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.02rem 0.34rem;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.76rem;
  }

  .hint-close {
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 1.05rem;
    line-height: 1;
    padding: 0 0.2rem;
  }

  .hint-close:hover {
    color: var(--text);
  }
</style>
