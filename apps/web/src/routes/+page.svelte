<script lang="ts">
  import { onMount } from 'svelte';
  import {
    EditorView,
    HomeView,
    ProjectView,
    type AudioInfo,
    type ProjectSummary,
    type RecordingEntry,
    type WasmColormapName
  } from '@phonix/ui';
  import { WasmCoreClient } from '$lib/core/WasmCoreClient';
  import { WebAudioPlayback } from '$lib/playback/WebAudioPlayback';
  import {
    AUTOSAVE_DEBOUNCE_MS,
    AUTOSAVE_MAX_WAIT_MS,
    ProjectStore,
    type ProjectState
  } from '$lib/project/ProjectStore';

  type Route = 'home' | 'project' | 'editor';

  let client = $state<WasmCoreClient | null>(null);
  let store = $state<ProjectStore | null>(null);
  let playback = $state<WebAudioPlayback | null>(null);

  let route = $state<Route>('home');
  let projects = $state<ProjectSummary[]>([]);
  let project = $state<ProjectState | null>(null);
  let recording = $state<RecordingEntry | null>(null);

  let audio = $state<AudioInfo | null>(null);
  let annotationId = $state<bigint | null>(null);
  let cursorTime = $state(0);
  let isPlaying = $state(false);
  let theme = $state<'light' | 'dark'>('light');
  let colormap = $state<WasmColormapName>('Viridis');
  let error = $state('');
  let busy = $state(false);
  let busyLabel = $state('');
  let dirty = $state(false);
  let recovery = $state<{ id: string; name: string } | null>(null);

  // Autosave debounce, driven from a coarse tick against the engine state hash.
  let lastHash: bigint | null = null;
  let pendingSince: number | null = null;
  let lastChange = 0;
  let autosaveBusy = false;
  let frame = 0;
  let saveTimer: ReturnType<typeof setInterval> | null = null;

  onMount(() => {
    client = new WasmCoreClient();
    store = new ProjectStore(client);
    playback = new WebAudioPlayback();
    const saved = localStorage.getItem('phonix-theme');
    theme =
      saved === 'dark' || saved === 'light'
        ? saved
        : window.matchMedia('(prefers-color-scheme: dark)').matches
          ? 'dark'
          : 'light';
    applyTheme(theme);
    void refreshProjects();

    const tick = () => {
      if (playback) {
        cursorTime = playback.position;
        isPlaying = playback.playing;
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    saveTimer = setInterval(() => void autosaveTick(), 500);

    return () => {
      cancelAnimationFrame(frame);
      if (saveTimer) clearInterval(saveTimer);
      client?.destroy();
      playback?.close();
    };
  });

  function applyTheme(next: 'light' | 'dark') {
    document.documentElement.classList.toggle('dark', next === 'dark');
    localStorage.setItem('phonix-theme', next);
  }

  function handleThemeChange(next: 'light' | 'dark') {
    theme = next;
    applyTheme(next);
  }

  async function refreshProjects() {
    if (!store) return;
    try {
      projects = await store.list();
    } catch (caught) {
      report(caught);
    }
  }

  function report(caught: unknown) {
    error = caught instanceof Error ? caught.message : String(caught);
  }

  function deriveName(files: File[]): string {
    for (const file of files) {
      const rel = (file as File & { webkitRelativePath?: string }).webkitRelativePath;
      if (rel && rel.includes('/')) return rel.split('/')[0];
    }
    const wav = files.find((file) => file.name.toLowerCase().endsWith('.wav'));
    if (wav) return wav.name.replace(/\.[^.]+$/, '');
    return 'Untitled project';
  }

  async function importToNewProject(files: File[]) {
    if (!store) return;
    error = '';
    busy = true;
    busyLabel = 'Importing recordings…';
    try {
      const created = await store.create(deriveName(files));
      project = created;
      route = 'project';
      await store.importFiles(created, files, () => {
        project = { ...created };
      });
      project = { ...created };
      resetAutosaveBaseline();
      await refreshProjects();
    } catch (caught) {
      report(caught);
    } finally {
      busy = false;
    }
  }

  async function createEmptyProject(name: string) {
    if (!store) return;
    error = '';
    try {
      const created = await store.create(name);
      project = created;
      route = 'project';
      resetAutosaveBaseline();
      await refreshProjects();
    } catch (caught) {
      report(caught);
    }
  }

  function requestOpen(id: string) {
    const summary = projects.find((entry) => entry.id === id);
    if (summary?.hasRecovery) {
      recovery = { id, name: summary.name };
      return;
    }
    void doOpen(id);
  }

  async function doOpen(id: string) {
    if (!store) return;
    error = '';
    busy = true;
    busyLabel = 'Opening project…';
    try {
      const result = await store.open(id);
      project = result.project;
      route = 'project';
      dirty = false;
      resetAutosaveBaseline();
      await refreshProjects();
    } catch (caught) {
      report(caught);
    } finally {
      busy = false;
    }
  }

  async function recoverAccept() {
    const target = recovery;
    recovery = null;
    if (target) await doOpen(target.id);
  }

  async function recoverDiscard() {
    const target = recovery;
    recovery = null;
    if (target && store) {
      await store.discardRecovery(target.id);
      await doOpen(target.id);
    }
  }

  async function addFilesToProject(files: File[]) {
    if (!store || !project) return;
    error = '';
    busy = true;
    busyLabel = 'Importing recordings…';
    const current = project;
    try {
      await store.importFiles(current, files, () => {
        project = { ...current };
      });
      project = { ...current };
      resetAutosaveBaseline();
      await refreshProjects();
    } catch (caught) {
      report(caught);
    } finally {
      busy = false;
    }
  }

  async function openRecording(entry: RecordingEntry) {
    if (!client || !store || !project) return;
    error = '';
    try {
      if (entry.audioId === null) return;
      if (entry.annotationId === null) {
        entry.annotationId = await client.createAnnotation(entry.audioId, 0, entry.duration);
        entry.hasAnnotation = true;
        project = { ...project };
      }
      recording = entry;
      audio = {
        id: entry.audioId,
        duration: entry.duration,
        sampleRate: entry.sampleRate,
        channels: entry.channels,
        name: entry.name,
        hash: entry.hash
      };
      annotationId = entry.annotationId;
      cursorTime = 0;
      const file = await store.readAudioFile(project.id, entry);
      if (file) await playback?.load(file);
      playback?.seek(0);
      resetAutosaveBaseline();
      route = 'editor';
    } catch (caught) {
      report(caught);
    }
  }

  function switchRecording(mediaId: number) {
    const entry = project?.recordings.find((item) => item.mediaId === mediaId);
    if (entry) void openRecording(entry);
  }

  async function editorImportFile(file: File) {
    if (!store || !project) return;
    error = '';
    busy = true;
    busyLabel = 'Importing recording…';
    const current = project;
    try {
      const before = current.recordings.length;
      await store.importFiles(current, [file], () => {
        project = { ...current };
      });
      project = { ...current };
      resetAutosaveBaseline();
      await refreshProjects();
      const added = current.recordings[before] ?? current.recordings.at(-1);
      if (added) await openRecording(added);
    } catch (caught) {
      report(caught);
    } finally {
      busy = false;
    }
  }

  async function handlePlayToggle() {
    if (!playback || !audio) return;
    error = '';
    try {
      isPlaying = await playback.toggle(cursorTime);
    } catch (caught) {
      report(caught);
    }
  }

  function handleCursorChange(time: number) {
    cursorTime = time;
    playback?.seek(time);
  }

  function backToProject() {
    route = 'project';
  }

  function backToHome() {
    void refreshProjects();
    route = 'home';
  }

  async function saveProject() {
    if (!store || !project) return;
    try {
      await store.writeProjectFile(project);
      dirty = false;
      pendingSince = null;
      await refreshProjects();
    } catch (caught) {
      report(caught);
    }
  }

  async function deleteProject(id: string) {
    if (!store) return;
    try {
      await store.delete(id);
      await refreshProjects();
    } catch (caught) {
      report(caught);
    }
  }

  async function renameProject(id: string, currentName: string) {
    if (!store) return;
    const next = window.prompt('Rename project', currentName);
    if (next === null) return;
    try {
      await store.rename(id, next);
      await refreshProjects();
    } catch (caught) {
      report(caught);
    }
  }

  async function duplicateProject(id: string) {
    if (!store) return;
    try {
      await store.duplicate(id);
      await refreshProjects();
    } catch (caught) {
      report(caught);
    }
  }

  function resetAutosaveBaseline() {
    lastHash = null;
    pendingSince = null;
  }

  async function autosaveTick() {
    if (!client || !store || !project || autosaveBusy) return;
    if (route === 'home') return;
    autosaveBusy = true;
    try {
      const hash = await client.stateHash();
      const now = Date.now();
      if (lastHash === null) {
        lastHash = hash;
      } else if (hash !== lastHash) {
        lastHash = hash;
        lastChange = now;
        pendingSince ??= now;
        dirty = true;
      }
      if (pendingSince !== null) {
        const quiet = now - lastChange >= AUTOSAVE_DEBOUNCE_MS;
        const waited = now - pendingSince >= AUTOSAVE_MAX_WAIT_MS;
        if (quiet || waited) {
          pendingSince = null;
          await store.writeAutosave(project);
          await refreshProjects();
        }
      }
    } catch (caught) {
      report(caught);
    } finally {
      autosaveBusy = false;
    }
  }

  const recordingChoices = $derived(
    project?.recordings.map((entry) => ({ mediaId: entry.mediaId, name: entry.name })) ?? []
  );

  // Test hook: the batch-equals-GUI invariant check reads the live client and
  // the open recording's audio id to run a direct engine query at the same
  // coordinates the readout used.
  $effect(() => {
    (globalThis as unknown as { __phonix?: unknown }).__phonix = {
      client,
      audioId: audio?.id ?? null
    };
  });
</script>

{#if route === 'home'}
  <HomeView
    {projects}
    {busy}
    {busyLabel}
    {theme}
    onImportFiles={importToNewProject}
    onNewProject={createEmptyProject}
    onOpenProject={requestOpen}
    onRenameProject={renameProject}
    onDeleteProject={deleteProject}
    onDuplicateProject={duplicateProject}
    onThemeChange={handleThemeChange}
  />
{:else if route === 'project' && project}
  <ProjectView
    {client}
    projectName={project.name}
    recordings={project.recordings}
    {theme}
    {busy}
    {busyLabel}
    {dirty}
    onOpenRecording={openRecording}
    onImportFiles={addFilesToProject}
    onBack={backToHome}
    onSave={saveProject}
    onThemeChange={handleThemeChange}
  />
{:else if route === 'editor'}
  <EditorView
    {client}
    {audio}
    {annotationId}
    {cursorTime}
    {isPlaying}
    {theme}
    {colormap}
    onFile={editorImportFile}
    onPlayToggle={handlePlayToggle}
    onThemeChange={handleThemeChange}
    onColormapChange={(next) => (colormap = next)}
    onCursorChange={handleCursorChange}
    onAnnotationChange={(id) => {
      annotationId = id;
      if (recording) recording.annotationId = id;
    }}
    onExit={backToProject}
    projectName={project?.name}
    recordings={recordingChoices}
    currentRecordingId={recording?.mediaId ?? null}
    onSwitchRecording={switchRecording}
    onPlaySelection={(t0, t1) => {
      cursorTime = t0;
      void playback?.playRange(t0, t1);
    }}
  />
{/if}

{#if recovery}
  <div class="modal-backdrop" data-testid="recovery-prompt">
    <div class="modal" role="dialog" aria-modal="true" aria-label="Recover unsaved work">
      <h2>Recover unsaved work?</h2>
      <p>
        “{recovery.name}” has autosaved changes from a session that did not finish. Recover them, or
        discard and open the last saved version.
      </p>
      <div class="modal-actions">
        <button type="button" class="secondary" data-testid="recovery-discard" onclick={recoverDiscard}>
          Discard
        </button>
        <button type="button" class="primary" data-testid="recovery-accept" onclick={recoverAccept}>
          Recover
        </button>
      </div>
    </div>
  </div>
{/if}

{#if error}
  <div class="error" role="alert" data-testid="error">{error}</div>
{/if}

<style>
  .error {
    position: fixed;
    right: 1rem;
    bottom: 1rem;
    max-width: min(30rem, calc(100vw - 2rem));
    padding: 0.75rem 0.9rem;
    border: 1px solid color-mix(in oklab, var(--warn), transparent 30%);
    border-radius: 6px;
    background: var(--panel);
    color: var(--warn);
    box-shadow: 0 12px 32px rgba(15, 23, 42, 0.16);
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(15, 23, 42, 0.42);
    z-index: 20;
  }

  .modal {
    max-width: 26rem;
    padding: 1.25rem 1.4rem;
    border: 1px solid var(--chrome-strong);
    border-radius: 12px;
    background: var(--panel);
    color: var(--text);
    box-shadow: 0 24px 60px rgba(15, 23, 42, 0.3);
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
    border-radius: 6px;
    padding: 0.4rem 0.9rem;
    background: var(--panel-soft);
    color: var(--text);
  }

  .modal-actions .primary {
    border-color: var(--accent);
    background: color-mix(in oklab, var(--accent) 22%, var(--panel-soft));
  }
</style>
