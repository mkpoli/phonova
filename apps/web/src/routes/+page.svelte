<script lang="ts">
  import { onMount } from 'svelte';
  import { base } from '$app/paths';
  import {
    CommandPalette,
    EditorView,
    HomeView,
    ProjectView,
    RecordingStrip,
    provideCommandRegistry,
    registerCommands,
    createGroup as treeCreateGroup,
    renameGroup as treeRenameGroup,
    dissolveGroup as treeDissolveGroup,
    moveNode as treeMoveNode,
    type AudioExportRequest,
    type AudioInfo,
    type HomeIndex,
    type LibraryNode,
    type ProjectExportMode,
    type ProjectSummary,
    type RecordingEntry,
    DEFAULT_PALETTE,
    loadCustomRamps,
    saveCustomRamps,
    type CustomRamp,
    type PaletteSelection
  } from '@phonia/ui';
  import { WasmCoreClient } from '$lib/core/WasmCoreClient';
  import { WebAudioPlayback } from '$lib/playback/WebAudioPlayback';
  import { MicRecorder, canRecord, type RecorderDevice, type RecorderLevel } from '$lib/audio/MicRecorder';
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
  let homeIndex = $state<HomeIndex>({ pinned: [], groups: [] });
  let project = $state<ProjectState | null>(null);
  let recording = $state<RecordingEntry | null>(null);

  let audio = $state<AudioInfo | null>(null);
  let annotationId = $state<bigint | null>(null);
  let cursorTime = $state(0);
  let isPlaying = $state(false);
  let theme = $state<'light' | 'dark'>('light');
  // The active spectrogram palette (default the brand ramp) and the machine's
  // saved custom ramps. Both persist in localStorage, app-wide.
  let palette = $state<PaletteSelection>(DEFAULT_PALETTE);
  let customRamps = $state<CustomRamp[]>([]);
  // App-wide UI scale as a fraction of the base root font size. rem-based layout
  // grows and shrinks with it; the bounds keep both extremes usable.
  let uiScale = $state(1);
  const UI_SCALE_MIN = 0.9;
  const UI_SCALE_MAX = 1.5;
  const UI_SCALE_STEP = 0.1;
  const UI_SCALE_BASE_PX = 16;
  let error = $state('');
  let busy = $state(false);
  let busyLabel = $state('');
  let dirty = $state(false);
  let recovery = $state<{ id: string; name: string } | null>(null);

  // Deletion runs through the journaled detach; the row hides during the undo
  // window and the OPFS files are purged only when the project is saved.
  //
  // The toast's Undo action must target the delete's own journal entry, not
  // whatever the journal head happens to be when it's clicked: any other
  // journaled edit inside the 8-second window would otherwise be the thing
  // that actually gets undone. `journalEntryId` is the id captured right
  // after the delete; `stale` flips true once a later check finds the head
  // has moved on, at which point the button stops calling undo() at all.
  let pendingRemovals = $state<number[]>([]);
  let removalUndo = $state<{
    mediaId: number;
    name: string;
    journalEntryId: bigint | null;
    stale: boolean;
  } | null>(null);
  let removalTimer: ReturnType<typeof setTimeout> | null = null;

  function collapsedOf(target: ProjectState): number[] {
    const view = target.view as { collapsedGroups?: number[] } | null;
    return Array.isArray(view?.collapsedGroups) ? view.collapsedGroups : [];
  }

  function clearPendingRemovals() {
    pendingRemovals = [];
    removalUndo = null;
    if (removalTimer) clearTimeout(removalTimer);
    removalTimer = null;
  }

  // Microphone recording. The recorder lives on the main thread and forwards
  // planar chunks to the engine worker; the strip reads the meter and elapsed
  // time. `recordingSupported` gates the Record controls so the desktop shell
  // (no getUserMedia) simply never shows them.
  let recordingSupported = $state(false);
  let recorder: MicRecorder | null = null;
  let capturing = $state(false);
  let recordingId: bigint | null = null;
  let recordingName = '';
  let recordStartMs = 0;
  let recordDevices = $state<RecorderDevice[]>([]);
  let recordDeviceId = $state('');
  let recordLevel = $state<RecorderLevel>({ rms: 0, peak: 0, clipped: false });
  let recordClipLatched = $state(false);
  let recordElapsed = $state(0);
  let recordSampleRate = $state(0);
  // True while capturing into a project that this take just created (recording
  // started from Home with no project open), so the strip can name it plainly.
  let recordDestinationNew = $state(false);

  const commands = provideCommandRegistry();

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

    customRamps = loadCustomRamps();
    palette = loadPalette(customRamps);

    const savedScale = Number(localStorage.getItem('phonix-ui-scale'));
    uiScale = Number.isFinite(savedScale) && savedScale > 0 ? clampScale(savedScale) : 1;
    applyUiScale(uiScale);

    void refreshProjects();

    recordingSupported = canRecord();
    if (recordingSupported) recorder = new MicRecorder(`${base}/recorder-worklet.js`);

    const tick = () => {
      if (playback) {
        cursorTime = playback.position;
        isPlaying = playback.playing;
      }
      if (capturing) recordElapsed = (performance.now() - recordStartMs) / 1000;
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    saveTimer = setInterval(() => void autosaveTick(), 500);

    return () => {
      cancelAnimationFrame(frame);
      if (saveTimer) clearInterval(saveTimer);
      recorder?.cancel();
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

  const PALETTE_KEY = 'phonia:palette';

  // Restore the saved palette, resolving a custom selection against the live
  // ramp list so an edited ramp reloads with its current stops. Falls back to
  // the default when the saved ramp was deleted or nothing was stored.
  function loadPalette(ramps: CustomRamp[]): PaletteSelection {
    try {
      const raw = localStorage.getItem(PALETTE_KEY);
      if (!raw) return DEFAULT_PALETTE;
      const saved = JSON.parse(raw) as { kind: string; name?: string; id?: string };
      if (saved.kind === 'custom' && saved.id) {
        const ramp = ramps.find((r) => r.id === saved.id);
        return ramp ? { kind: 'custom', ramp } : DEFAULT_PALETTE;
      }
      if (saved.kind === 'builtin' && saved.name) {
        return { kind: 'builtin', name: saved.name } as PaletteSelection;
      }
    } catch {
      // Unreadable selection: the default ramp stands.
    }
    return DEFAULT_PALETTE;
  }

  function persistPalette(sel: PaletteSelection) {
    try {
      const ref =
        sel.kind === 'custom' ? { kind: 'custom', id: sel.ramp.id } : { kind: 'builtin', name: sel.name };
      localStorage.setItem(PALETTE_KEY, JSON.stringify(ref));
    } catch {
      // Storage unavailable: the selection stays for the session.
    }
  }

  function handlePaletteChange(next: PaletteSelection) {
    palette = next;
    persistPalette(next);
  }

  // Persist a created or edited ramp, keeping the list keyed by id, and refresh
  // the active selection if it names the same ramp.
  function saveRamp(ramp: CustomRamp) {
    const idx = customRamps.findIndex((r) => r.id === ramp.id);
    customRamps =
      idx >= 0
        ? customRamps.map((r) => (r.id === ramp.id ? ramp : r))
        : [...customRamps, ramp];
    saveCustomRamps(customRamps);
    if (palette.kind === 'custom' && palette.ramp.id === ramp.id) {
      palette = { kind: 'custom', ramp };
    }
  }

  function deleteRamp(id: string) {
    customRamps = customRamps.filter((r) => r.id !== id);
    saveCustomRamps(customRamps);
    if (palette.kind === 'custom' && palette.ramp.id === id) {
      handlePaletteChange(DEFAULT_PALETTE);
    }
  }

  function clampScale(value: number): number {
    return Math.min(UI_SCALE_MAX, Math.max(UI_SCALE_MIN, Math.round(value * 100) / 100));
  }

  function applyUiScale(next: number) {
    document.documentElement.style.fontSize = `${(UI_SCALE_BASE_PX * next).toFixed(3)}px`;
    localStorage.setItem('phonix-ui-scale', String(next));
  }

  function setUiScale(next: number) {
    uiScale = clampScale(next);
    applyUiScale(uiScale);
  }

  function nudgeUiScale(direction: number) {
    setUiScale(uiScale + direction * UI_SCALE_STEP);
  }

  function resetUiScale() {
    setUiScale(1);
  }

  registerCommands([
    {
      id: 'switchTheme',
      title: 'Switch color theme',
      group: 'Appearance',
      keywords: ['dark', 'light', 'appearance', 'toggle theme'],
      run: () => handleThemeChange(theme === 'light' ? 'dark' : 'light')
    },
    {
      id: 'uiScaleUp',
      title: 'Increase UI scale',
      group: 'Appearance',
      shortcut: 'Ctrl/Cmd++',
      keywords: ['zoom interface', 'font size', 'bigger', 'text size'],
      run: () => nudgeUiScale(1)
    },
    {
      id: 'uiScaleDown',
      title: 'Decrease UI scale',
      group: 'Appearance',
      shortcut: 'Ctrl/Cmd+-',
      keywords: ['zoom interface', 'font size', 'smaller', 'text size'],
      run: () => nudgeUiScale(-1)
    },
    {
      id: 'uiScaleReset',
      title: 'Reset UI scale',
      group: 'Appearance',
      shortcut: 'Ctrl/Cmd+0',
      keywords: ['zoom interface', 'font size', 'default'],
      run: resetUiScale
    }
  ]);

  async function refreshProjects() {
    if (!store) return;
    try {
      projects = await store.list();
      await loadHomeIndex();
    } catch (caught) {
      report(caught);
    }
  }

  /**
   * Reads the home index and drops any pin or group membership naming a project
   * that no longer exists, so a deleted project leaves no dangling reference.
   * Writes back only when pruning changed something.
   */
  async function loadHomeIndex() {
    if (!store) return;
    const known = new Set(projects.map((p) => p.id));
    const raw = await store.readHomeIndex();
    const pinned = raw.pinned.filter((id) => known.has(id));
    const groups = raw.groups.map((g) => ({
      ...g,
      members: g.members.filter((id) => known.has(id))
    }));
    const pruned =
      pinned.length !== raw.pinned.length ||
      groups.some((g, i) => g.members.length !== raw.groups[i].members.length);
    homeIndex = { pinned, groups };
    if (pruned) await store.writeHomeIndex(homeIndex);
  }

  async function updateHomeIndex(next: HomeIndex) {
    if (!store) return;
    homeIndex = next;
    try {
      await store.writeHomeIndex(next);
    } catch (caught) {
      report(caught);
    }
  }

  function togglePin(id: string) {
    const pinned = homeIndex.pinned.includes(id)
      ? homeIndex.pinned.filter((x) => x !== id)
      : [...homeIndex.pinned, id];
    void updateHomeIndex({ ...homeIndex, pinned });
  }

  // A project lives in one group: seeding or moving into a group first drops it
  // from every other group's membership.
  function withoutMembers(groups: HomeIndex['groups'], ids: string[]): HomeIndex['groups'] {
    const drop = new Set(ids);
    return groups.map((g) => ({ ...g, members: g.members.filter((m) => !drop.has(m)) }));
  }

  function createGroupFrom(memberIds: string[]) {
    const group = {
      id: crypto.randomUUID(),
      name: 'New group',
      members: [...memberIds],
      collapsed: false
    };
    const groups = [...withoutMembers(homeIndex.groups, memberIds), group];
    void updateHomeIndex({ ...homeIndex, groups });
  }

  function renameHomeGroup(groupId: string, name: string) {
    const trimmed = name.trim();
    if (!trimmed) return;
    const groups = homeIndex.groups.map((g) => (g.id === groupId ? { ...g, name: trimmed } : g));
    void updateHomeIndex({ ...homeIndex, groups });
  }

  function dissolveHomeGroup(groupId: string) {
    const groups = homeIndex.groups.filter((g) => g.id !== groupId);
    void updateHomeIndex({ ...homeIndex, groups });
  }

  function toggleGroupCollapse(groupId: string) {
    const groups = homeIndex.groups.map((g) =>
      g.id === groupId ? { ...g, collapsed: !g.collapsed } : g
    );
    void updateHomeIndex({ ...homeIndex, groups });
  }

  function moveToGroup(id: string, groupId: string | null) {
    let groups = withoutMembers(homeIndex.groups, [id]);
    if (groupId !== null) {
      groups = groups.map((g) => (g.id === groupId ? { ...g, members: [...g.members, id] } : g));
    }
    void updateHomeIndex({ ...homeIndex, groups });
  }

  async function exportStoredProject(id: string) {
    if (!store) return;
    try {
      const { name, bytes } = await store.exportStored(id);
      downloadBytes(bytes, `${sanitizeFileName(name)}.phxproj`, 'application/zip');
    } catch (caught) {
      report(caught);
    }
  }

  async function batchDeleteProjects(ids: string[]) {
    if (!store || ids.length === 0) return;
    error = '';
    busy = true;
    busyLabel = `Deleting ${ids.length} ${ids.length === 1 ? 'project' : 'projects'}…`;
    try {
      for (const id of ids) await store.delete(id);
      await refreshProjects();
    } catch (caught) {
      report(caught);
    } finally {
      busy = false;
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

  interface SampleManifest {
    name: string;
    files: Array<{ path: string; name: string; mime: string }>;
  }

  async function openSampleProject() {
    if (!store) return;
    error = '';
    busy = true;
    busyLabel = 'Loading sample project…';
    try {
      const manifest: SampleManifest = await fetch(`${base}/sample/manifest.json`).then((res) => {
        if (!res.ok) throw new Error('Sample project manifest is unavailable.');
        return res.json();
      });
      const files = await Promise.all(
        manifest.files.map(async (entry) => {
          const res = await fetch(`${base}/sample/${entry.path}`);
          if (!res.ok) throw new Error(`Sample file ${entry.path} is unavailable.`);
          return new File([await res.arrayBuffer()], entry.name, { type: entry.mime });
        })
      );
      const created = await store.create(manifest.name);
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
      clearPendingRemovals();
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
    clearPendingRemovals();
    void refreshProjects();
    route = 'home';
  }

  async function saveProject() {
    if (!store || !project) return;
    try {
      if (pendingRemovals.length > 0) {
        await store.finalizeRemovals(project, pendingRemovals);
        clearPendingRemovals();
      }
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

  async function renameProject(id: string, name: string) {
    if (!store) return;
    try {
      await store.rename(id, name);
      if (project?.id === id) project = { ...project, name: name.trim() || project.name };
      await refreshProjects();
    } catch (caught) {
      report(caught);
    }
  }

  async function renameRecording(mediaId: number, name: string) {
    if (!store || !project) return;
    try {
      await store.renameRecording(project, mediaId, name);
      project = { ...project };
      if (recording?.mediaId === mediaId) {
        recording = project.recordings.find((entry) => entry.mediaId === mediaId) ?? recording;
        if (audio && recording) audio = { ...audio, name: recording.name };
      }
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

  // --- Project and audio I/O ---

  let notice = $state('');

  function sanitizeFileName(name: string): string {
    const cleaned = name.replace(/[\\/:*?"<>|]/g, '_').trim();
    return cleaned || 'untitled';
  }

  function downloadBytes(bytes: Uint8Array, fileName: string, mime: string) {
    const owned = new Uint8Array(bytes.byteLength);
    owned.set(bytes);
    const blob = new Blob([owned], { type: mime });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = fileName;
    anchor.click();
    URL.revokeObjectURL(url);
  }

  async function exportProject(mode: ProjectExportMode) {
    if (!store || !project) return;
    error = '';
    busy = true;
    busyLabel = mode === 'bundle' ? 'Building bundle…' : 'Exporting project…';
    try {
      const bytes = await store.exportProject(project, mode);
      downloadBytes(bytes, `${sanitizeFileName(project.name)}.phxproj`, 'application/zip');
    } catch (caught) {
      report(caught);
    } finally {
      busy = false;
    }
  }

  async function openProjectFile(file: File) {
    if (!store) return;
    error = '';
    notice = '';
    busy = true;
    busyLabel = 'Opening project file…';
    try {
      const result = await store.importProjectFile(file);
      project = result.project;
      route = 'project';
      dirty = false;
      clearPendingRemovals();
      resetAutosaveBaseline();
      await refreshProjects();
      if (result.gaps.length > 0) {
        const names = result.gaps.map((gap) => gap.name).join(', ');
        notice = `Imported. ${result.gaps.length} recording(s) could not be located: ${names}. Re-link them by adding the source audio.`;
      }
    } catch (caught) {
      report(caught);
    } finally {
      busy = false;
    }
  }

  async function exportRecordingAudio(mediaId: number) {
    if (!client || !project) return;
    const entry = project.recordings.find((r) => r.mediaId === mediaId);
    if (!entry || entry.audioId === null) return;
    error = '';
    try {
      const bytes = await client.exportSpanWav(entry.audioId, 0, entry.duration, 'Pcm16');
      downloadBytes(bytes, `${sanitizeFileName(entry.name)}.wav`, 'audio/wav');
    } catch (caught) {
      report(caught);
    }
  }

  async function exportEditorAudio(request: AudioExportRequest) {
    if (!client || !audio) return;
    error = '';
    try {
      const bytes = request.filtered
        ? await client.exportBandFilteredSpanWav(
            audio.id,
            request.t0,
            request.t1,
            request.f0,
            request.f1,
            request.bits
          )
        : await client.exportSpanWav(audio.id, request.t0, request.t1, request.bits);
      const base = sanitizeFileName(recording?.name ?? audio.name ?? 'audio');
      const suffix = request.scope === 'selection' ? '-selection' : '';
      downloadBytes(bytes, `${base}${suffix}.wav`, 'audio/wav');
    } catch (caught) {
      report(caught);
    }
  }

  // --- Library tree ---

  async function applyLibrary(next: LibraryNode[]) {
    if (!store || !project) return;
    try {
      await store.updateLibrary(project, next);
      project = { ...project };
    } catch (caught) {
      report(caught);
    }
  }

  function createGroup() {
    if (!project) return;
    void applyLibrary(treeCreateGroup(project.groups, 'New group', null));
  }

  function renameGroup(groupId: number, name: string) {
    if (!project) return;
    void applyLibrary(treeRenameGroup(project.groups, groupId, name));
  }

  function dissolveGroup(groupId: number) {
    if (!project) return;
    void applyLibrary(treeDissolveGroup(project.groups, groupId));
  }

  function moveNode(key: string, targetGroupId: number | null, index: number) {
    if (!project) return;
    void applyLibrary(treeMoveNode(project.groups, key, targetGroupId, index));
  }

  async function toggleCollapse(groupId: number) {
    if (!store || !project) return;
    const current = collapsedOf(project);
    const next = current.includes(groupId)
      ? current.filter((id) => id !== groupId)
      : [...current, groupId];
    const view = { ...((project.view as object | null) ?? {}), collapsedGroups: next };
    try {
      await store.updateView(project, view);
      project = { ...project };
    } catch (caught) {
      report(caught);
    }
  }

  // --- Metadata ---

  async function updateRecordingMetadata(
    mediaId: number,
    metadata: { description: string; authors: string[]; tags: string[] }
  ) {
    if (!store || !project) return;
    try {
      await store.updateRecordingMetadata(project, mediaId, metadata);
      project = { ...project };
    } catch (caught) {
      report(caught);
    }
  }

  async function updateProjectMetadata(metadata: {
    description: string;
    authors: string[];
    tags: string[];
  }) {
    if (!store || !project) return;
    try {
      await store.updateProjectMetadata(project, metadata);
      project = { ...project };
    } catch (caught) {
      report(caught);
    }
  }

  // --- Delete with undo ---

  async function deleteRecording(mediaId: number) {
    if (!client || !project) return;
    const entry = project.recordings.find((r) => r.mediaId === mediaId);
    if (!entry) return;
    try {
      let journalEntryId: bigint | null = null;
      if (entry.audioId !== null) {
        await client.detachAudio(entry.audioId);
        // The detach just applied is the journal head; capture its id so the
        // toast can later confirm it is still the entry undo() would target.
        journalEntryId = await client.journalHeadId();
      }
      // The detach cascaded the annotation off the session; drop the reference so
      // an autosave inside the undo window does not serialize a removed document.
      entry.annotationId = null;
      entry.hasAnnotation = false;
      pendingRemovals = [...pendingRemovals, mediaId];
      removalUndo = { mediaId, name: entry.name, journalEntryId, stale: false };
      project = { ...project };
      if (removalTimer) clearTimeout(removalTimer);
      removalTimer = setTimeout(() => (removalUndo = null), 8000);
    } catch (caught) {
      report(caught);
    }
  }

  async function undoRemoval() {
    if (!client || !project || !removalUndo || removalUndo.stale) return;
    const target = removalUndo;
    try {
      const head = target.journalEntryId === null ? null : await client.journalHeadId();
      if (target.journalEntryId === null || head !== target.journalEntryId) {
        // Something else was journaled since the delete (or there was never
        // anything to undo); a blind undo() would hit that entry instead of
        // restoring this recording. Stop offering the action rather than
        // undo the wrong thing.
        removalUndo = { ...target, stale: true };
        return;
      }
      if (removalTimer) clearTimeout(removalTimer);
      removalTimer = null;
      removalUndo = null;
      await client.undo();
      const entry = project.recordings.find((r) => r.mediaId === target.mediaId);
      if (entry && entry.audioId !== null) {
        const anns = await client.listAnnotations(entry.audioId);
        entry.annotationId = anns.length ? anns[anns.length - 1] : null;
        entry.hasAnnotation = anns.length > 0;
      }
      pendingRemovals = pendingRemovals.filter((id) => id !== target.mediaId);
      project = { ...project };
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

  function timestampName(): string {
    const now = new Date();
    const pad = (value: number) => String(value).padStart(2, '0');
    return (
      `Recording ${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ` +
      `${pad(now.getHours())}.${pad(now.getMinutes())}.${pad(now.getSeconds())}`
    );
  }

  function sessionName(): string {
    const now = new Date();
    const pad = (value: number) => String(value).padStart(2, '0');
    return (
      `Recordings ${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ` +
      `${pad(now.getHours())}.${pad(now.getMinutes())}`
    );
  }

  function handleRecordLevel(level: RecorderLevel) {
    recordLevel = level;
    if (level.clipped) recordClipLatched = true;
  }

  async function startRecording() {
    if (!client || !store || !recorder || capturing) return;
    error = '';
    try {
      // Recording always lands in a project; make one, named from the moment,
      // on the home screen so the take has a home the strip can announce.
      if (!project) {
        const created = await store.create(sessionName());
        project = created;
        recordDestinationNew = true;
        route = 'project';
        resetAutosaveBaseline();
        await refreshProjects();
      } else {
        recordDestinationNew = false;
      }
      recordingName = timestampName();
      recordClipLatched = false;
      recordLevel = { rms: 0, peak: 0, clipped: false };
      recordElapsed = 0;
      recordSampleRate = 0;

      // Buffer chunks that arrive before the engine take is open, then forward
      // in arrival order so no leading audio is lost to the startup race.
      let recId: bigint | null = null;
      const buffered: Float32Array[] = [];
      const forward = (samples: Float32Array) => {
        if (recId === null) buffered.push(samples);
        else void client?.appendSamples(recId, samples);
      };

      const started = await recorder.start({
        deviceId: recordDeviceId || undefined,
        onChunk: (chunk) => forward(chunk.samples),
        onLevel: handleRecordLevel
      });
      recordSampleRate = started.sampleRate;
      recId = await client.beginRecording(started.sampleRate, started.channels);
      for (const samples of buffered) void client.appendSamples(recId, samples);
      recordingId = recId;
      recordStartMs = performance.now();
      capturing = true;

      // Device labels are readable now that permission is granted.
      recordDevices = await recorder.listDevices();
      if (!recordDeviceId && recordDevices.length > 0) recordDeviceId = recordDevices[0].deviceId;
    } catch (caught) {
      recorder?.cancel();
      capturing = false;
      recordingId = null;
      report(caught);
    }
  }

  async function stopRecording() {
    if (!client || !store || !recorder || !capturing || recordingId === null || !project) return;
    const current = project;
    const recId = recordingId;
    const name = recordingName;
    try {
      await recorder.stop();
      const finished = await client.finishRecording(recId, name);
      const entry = await store.addRecording(current, name, finished);
      project = { ...current };
      capturing = false;
      recordingId = null;
      resetAutosaveBaseline();
      await refreshProjects();
      await openRecording(entry);
    } catch (caught) {
      capturing = false;
      recordingId = null;
      report(caught);
    }
  }

  async function cancelRecording() {
    if (!recorder || !capturing) return;
    const recId = recordingId;
    recorder.cancel();
    capturing = false;
    recordingId = null;
    try {
      if (recId !== null) await client?.abortRecording(recId);
    } catch (caught) {
      report(caught);
    }
  }

  function toggleRecording() {
    if (capturing) void stopRecording();
    else void startRecording();
  }

  async function selectRecordDevice(deviceId: string) {
    recordDeviceId = deviceId;
    // Switching devices mid-take restarts the capture graph on the new input
    // while the same engine take keeps accumulating.
    if (!capturing || !recorder || recordingId === null) return;
    const recId = recordingId;
    try {
      recorder.cancel();
      const started = await recorder.start({
        deviceId: deviceId || undefined,
        onChunk: (chunk) => void client?.appendSamples(recId, chunk.samples),
        onLevel: handleRecordLevel
      });
      recordSampleRate = started.sampleRate;
    } catch (caught) {
      report(caught);
    }
  }

  registerCommands([
    {
      id: 'startRecording',
      title: 'Start recording',
      group: 'Project',
      api: ['beginRecording'],
      shortcut: 'R',
      keywords: ['microphone', 'capture', 'mic', 'record'],
      enabled: () => recordingSupported && !capturing,
      run: () => void startRecording()
    },
    {
      id: 'stopRecording',
      title: 'Stop recording',
      group: 'Project',
      api: ['finishRecording'],
      shortcut: 'R',
      keywords: ['microphone', 'capture', 'mic', 'finish'],
      enabled: () => capturing,
      run: () => void stopRecording()
    }
  ]);

  function handleWindowKeydown(event: KeyboardEvent) {
    // App-wide UI scale on Ctrl/Cmd +/-/0, ahead of the record shortcut and
    // regardless of recording support. Preventing default also stops the
    // browser's own page zoom from firing.
    if (event.ctrlKey || event.metaKey) {
      if (event.key === '=' || event.key === '+') {
        event.preventDefault();
        nudgeUiScale(1);
        return;
      }
      if (event.key === '-' || event.key === '_') {
        event.preventDefault();
        nudgeUiScale(-1);
        return;
      }
      if (event.key === '0') {
        event.preventDefault();
        resetUiScale();
        return;
      }
    }
    if (!recordingSupported) return;
    if (event.key.toLowerCase() !== 'r' || event.metaKey || event.ctrlKey || event.altKey) return;
    const target = event.target;
    if (
      target instanceof HTMLInputElement ||
      target instanceof HTMLSelectElement ||
      target instanceof HTMLTextAreaElement
    ) {
      return;
    }
    event.preventDefault();
    toggleRecording();
  }

  const recordingChoices = $derived(
    project?.recordings.map((entry) => ({
      mediaId: entry.mediaId,
      name: entry.name,
      duration: entry.duration,
      audioId: entry.audioId,
      hasAnnotation: entry.hasAnnotation
    })) ?? []
  );

  // Test hook: the batch-equals-GUI invariant check reads the live client and
  // the open recording's audio id to run a direct engine query at the same
  // coordinates the readout used.
  $effect(() => {
    (globalThis as unknown as { __phonia?: unknown }).__phonia = {
      client,
      audioId: audio?.id ?? null
    };
  });
</script>

<svelte:window onkeydown={handleWindowKeydown} />

{#if route === 'home'}
  <HomeView
    {projects}
    {busy}
    {busyLabel}
    {theme}
    onImportFiles={importToNewProject}
    onNewProject={createEmptyProject}
    onOpenSample={openSampleProject}
    onOpenProjectFile={openProjectFile}
    onOpenProject={requestOpen}
    onRenameProject={renameProject}
    onDeleteProject={deleteProject}
    onDuplicateProject={duplicateProject}
    onThemeChange={handleThemeChange}
    onStartRecording={recordingSupported ? startRecording : undefined}
    recording={capturing}
    {homeIndex}
    onTogglePin={togglePin}
    onCreateGroupFrom={createGroupFrom}
    onRenameGroup={renameHomeGroup}
    onDissolveGroup={dissolveHomeGroup}
    onToggleGroupCollapse={toggleGroupCollapse}
    onMoveToGroup={moveToGroup}
    onExportStored={exportStoredProject}
    onBatchDelete={batchDeleteProjects}
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
    savedAt={project.savedAt}
    onOpenRecording={openRecording}
    onImportFiles={addFilesToProject}
    onBack={backToHome}
    onSave={saveProject}
    onThemeChange={handleThemeChange}
    onStartRecording={recordingSupported ? startRecording : undefined}
    recording={capturing}
    groups={project.groups}
    collapsed={collapsedOf(project)}
    onToggleCollapse={toggleCollapse}
    onCreateGroup={createGroup}
    onRenameGroup={renameGroup}
    onDissolveGroup={dissolveGroup}
    onMoveNode={moveNode}
    onRenameRecording={renameRecording}
    onDeleteRecording={deleteRecording}
    onExportProject={exportProject}
    onExportRecording={exportRecordingAudio}
    onUpdateRecordingMetadata={updateRecordingMetadata}
    onUpdateProjectMetadata={updateProjectMetadata}
    projectDescription={project.description}
    projectAuthors={project.authors}
    projectTags={project.tags}
    {pendingRemovals}
    {removalUndo}
    onUndoRemoval={undoRemoval}
  />
{:else if route === 'editor'}
  <EditorView
    {client}
    {audio}
    {annotationId}
    {cursorTime}
    {isPlaying}
    {theme}
    {palette}
    {customRamps}
    onFile={editorImportFile}
    onPlayToggle={handlePlayToggle}
    onThemeChange={handleThemeChange}
    onPaletteChange={handlePaletteChange}
    onSaveRamp={saveRamp}
    onDeleteRamp={deleteRamp}
    onCursorChange={handleCursorChange}
    onAnnotationChange={(id) => {
      annotationId = id;
      if (recording) {
        recording.annotationId = id;
        recording.hasAnnotation = id !== null;
      }
    }}
    onExit={backToProject}
    projectName={project?.name}
    recordings={recordingChoices}
    groups={project?.groups}
    currentRecordingId={recording?.mediaId ?? null}
    onSwitchRecording={switchRecording}
    onRenameRecording={renameRecording}
    onPlaySelection={(t0, t1) => {
      cursorTime = t0;
      void playback?.playRange(t0, t1);
    }}
    onPlayFilteredSelection={async (t0, t1, f0, f1) => {
      if (!client || !audio || !playback) return;
      try {
        const samples = await client.bandFilteredSpan(audio.id, t0, t1, f0, f1);
        await playback.playBuffer(samples, audio.sampleRate);
      } catch (caught) {
        report(caught);
      }
    }}
    onExportAudio={exportEditorAudio}
    onStartRecording={recordingSupported ? startRecording : undefined}
    recording={capturing}
    recordingElapsedSeconds={recordElapsed}
  />
{/if}

{#if capturing}
  <RecordingStrip
    devices={recordDevices}
    selectedDeviceId={recordDeviceId}
    level={recordLevel}
    clipLatched={recordClipLatched}
    elapsedSeconds={recordElapsed}
    sampleRate={recordSampleRate}
    destinationName={project?.name}
    destinationIsNew={recordDestinationNew}
    onRenameDestination={(name) => {
      if (project) void renameProject(project.id, name);
    }}
    onSelectDevice={selectRecordDevice}
    onStop={stopRecording}
    onCancel={cancelRecording}
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

{#if notice}
  <div class="notice" role="status" data-testid="notice">
    <span>{notice}</span>
    <button type="button" class="notice-close" aria-label="Dismiss" onclick={() => (notice = '')}>×</button>
  </div>
{/if}

<CommandPalette registry={commands} />

<style>
  .error {
    position: fixed;
    right: 1rem;
    bottom: 1rem;
    max-width: min(30rem, calc(100vw - 2rem));
    padding: 0.75rem 0.9rem;
    border: 1px solid color-mix(in oklab, var(--warn), transparent 30%);
    border-radius: var(--radius-md);
    background: var(--panel);
    color: var(--warn);
    box-shadow: var(--shadow-lg);
  }

  .notice {
    position: fixed;
    left: 50%;
    bottom: 1rem;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 0.75rem;
    max-width: min(38rem, calc(100vw - 2rem));
    padding: 0.6rem 0.7rem 0.6rem 0.95rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-md);
    background: var(--panel);
    color: var(--text);
    box-shadow: var(--shadow-lg);
    font-size: 0.85rem;
    z-index: 20;
  }

  .notice-close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 1.05rem;
    line-height: 1;
    padding: 0 0.2rem;
    cursor: pointer;
  }

  .notice-close:hover {
    color: var(--text);
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: color-mix(in oklab, #000 52%, transparent);
    backdrop-filter: blur(2px);
    z-index: 20;
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
    transition:
      background var(--t-fast),
      border-color var(--t-fast);
  }

  .modal-actions button:hover {
    background: var(--panel);
    border-color: color-mix(in oklab, var(--accent) 32%, var(--chrome-strong));
  }

  .modal-actions .primary {
    border-color: var(--action);
    background: var(--action);
    color: #fff;
  }

  .modal-actions .primary:hover {
    background: var(--action-strong);
    border-color: var(--action-strong);
  }
</style>
