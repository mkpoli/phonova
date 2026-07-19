import type { FinishedRecordingResult, WasmCoreClient } from '$lib/core/WasmCoreClient';
import { flatLibrary, pruneMedia } from '@phonia/ui';
import type {
  HomeIndex,
  HomeProjectGroup,
  LibraryNode,
  ProjectExportMode,
  ProjectSummary,
  RecordingEntry,
  SaveProjectMediaSpec,
  SaveProjectSpec
} from '$lib/core/types';

/** A recording an imported project references but whose media could not be found. */
export interface MissingMedia {
  mediaId: number;
  name: string;
  hash: string;
}

/** The outcome of importing a project file: the opened project and any media gaps. */
export interface ImportResult {
  project: ProjectState;
  gaps: MissingMedia[];
}

/** Directory under OPFS that holds one subdirectory per project. */
const PROJECTS_DIR = 'phonix-projects';
/**
 * Home-screen index file, beside the project directories.
 *
 * Schema (JSON):
 *   { "pinned": string[], "groups": [ { "id": string, "name": string,
 *     "members": string[], "collapsed": boolean } ] }
 *
 * `pinned` and each group's `members` hold project directory ids. A missing
 * file, unparseable content, or the wrong shape all read back as an empty
 * index; ids naming a project that no longer exists are simply ignored by the
 * home screen, so the file tolerates stale references without repair.
 */
const HOME_FILE = 'home.json';
/** Container file name inside a project directory. */
const PROJECT_FILE = 'project.phxproj';
/** Sidecar suffix, matching `phx_project::AUTOSAVE_SUFFIX`. */
const AUTOSAVE_SUFFIX = '.autosave';
/** Subdirectory holding the referenced recordings. */
const AUDIO_DIR = 'audio';

/**
 * Quiet period after the last edit before an autosave is written, in ms.
 * Mirrors `phx_project::DEFAULT_DEBOUNCE_MS`.
 */
export const AUTOSAVE_DEBOUNCE_MS = 2_000;
/**
 * Ceiling on how long unbroken editing defers an autosave, in ms.
 * Mirrors `phx_project::DEFAULT_MAX_WAIT_MS`.
 */
export const AUTOSAVE_MAX_WAIT_MS = 15_000;

/** An open project: its recordings bound to live session ids. */
export interface ProjectState {
  id: string;
  name: string;
  savedAt: number;
  recordings: RecordingEntry[];
  nextMediaId: number;
  view: unknown;
  /** Free-form description of the project. Empty when unset. */
  description: string;
  /** Contributors credited for the project, in listing order. */
  authors: string[];
  /** Free-form tags applied to the project, in listing order. */
  tags: string[];
  /** The library tree: the ordered root of a nesting of groups and recordings. */
  groups: LibraryNode[];
}

/** The recovery decision made when opening a project. */
export interface OpenResult {
  project: ProjectState;
  recovered: boolean;
}

function stem(fileName: string): string {
  const dot = fileName.lastIndexOf('.');
  return dot > 0 ? fileName.slice(0, dot) : fileName;
}

function extension(fileName: string): string {
  const dot = fileName.lastIndexOf('.');
  return dot >= 0 ? fileName.slice(dot + 1).toLowerCase() : '';
}

function isTextGrid(fileName: string): boolean {
  return extension(fileName) === 'textgrid';
}

/** Extensions of the audio containers the engine decodes: WAV, AIFF, FLAC. */
const AUDIO_EXTENSIONS = new Set(['wav', 'aiff', 'aif', 'flac']);

function isAudio(fileName: string): boolean {
  return AUDIO_EXTENSIONS.has(extension(fileName));
}

async function opfsRoot(): Promise<FileSystemDirectoryHandle> {
  const storage = navigator.storage;
  if (!storage?.getDirectory) {
    throw new Error('This browser does not expose the Origin Private File System.');
  }
  return storage.getDirectory();
}

async function projectsDir(create = false): Promise<FileSystemDirectoryHandle> {
  const root = await opfsRoot();
  return root.getDirectoryHandle(PROJECTS_DIR, { create });
}

async function projectDir(id: string, create = false): Promise<FileSystemDirectoryHandle> {
  const dir = await projectsDir(create);
  return dir.getDirectoryHandle(id, { create });
}

/** Copies bytes into a fresh `ArrayBuffer`-backed view the DOM APIs accept. */
function ownedBytes(bytes: Uint8Array): Uint8Array<ArrayBuffer> {
  const out = new Uint8Array(bytes.byteLength);
  out.set(bytes);
  return out;
}

async function readFileBytes(
  dir: FileSystemDirectoryHandle,
  name: string
): Promise<Uint8Array<ArrayBuffer> | null> {
  try {
    const handle = await dir.getFileHandle(name);
    const file = await handle.getFile();
    return new Uint8Array(await file.arrayBuffer());
  } catch {
    return null;
  }
}

async function writeFileBytes(
  dir: FileSystemDirectoryHandle,
  name: string,
  bytes: Uint8Array
): Promise<void> {
  const handle = await dir.getFileHandle(name, { create: true });
  const writable = await handle.createWritable();
  await writable.write(ownedBytes(bytes));
  await writable.close();
}

/**
 * Streams a File into OPFS without buffering it whole in memory.
 *
 * A large recording is piped straight from the File to the OPFS writable, so
 * importing an hour-long take never holds the file on the main thread; the
 * worker then opens it streamed off the persisted copy.
 */
async function writeFileStream(
  dir: FileSystemDirectoryHandle,
  name: string,
  file: File
): Promise<void> {
  const handle = await dir.getFileHandle(name, { create: true });
  const writable = await handle.createWritable();
  await file.stream().pipeTo(writable);
}

/** OPFS directory segments, from the root, that hold a project's recordings. */
function audioSegments(projectId: string): string[] {
  return [PROJECTS_DIR, projectId, AUDIO_DIR];
}

async function fileExists(dir: FileSystemDirectoryHandle, name: string): Promise<boolean> {
  try {
    await dir.getFileHandle(name);
    return true;
  } catch {
    return false;
  }
}

/**
 * Orchestrates project persistence over the Origin Private File System.
 *
 * The container format and its round-trip stay in `phx-project` (reached through
 * the worker's `saveProjectContainer` / `loadProjectContainer`); this class owns
 * only the OPFS tree — one directory per project holding the container, its
 * autosave sidecar, and the referenced `audio/` files — plus the recovery rule
 * (a sidecar strictly newer than the project file holds unsaved work) and the
 * autosave debounce timing, both matching `phx_project`.
 */
export class ProjectStore {
  #client: WasmCoreClient;
  // Container writes serialize through this chain: two mutations fired back to
  // back (create a group, then drag a recording into it) would otherwise race
  // on the one project file, and whichever finished last would win — losing the
  // earlier edit. The chain keeps each write ordered after the previous.
  #writeChain: Promise<void> = Promise.resolve();

  constructor(client: WasmCoreClient) {
    this.#client = client;
  }

  /** Runs `task` after any in-flight container write, serializing file writes. */
  #enqueueWrite(task: () => Promise<void>): Promise<void> {
    const next = this.#writeChain.then(task, task);
    this.#writeChain = next;
    return next;
  }

  /** Lists every stored project, newest first, flagging pending recovery. */
  async list(): Promise<ProjectSummary[]> {
    let dir: FileSystemDirectoryHandle;
    try {
      dir = await projectsDir(false);
    } catch {
      return [];
    }
    const summaries: ProjectSummary[] = [];
    for await (const [id, handle] of entries(dir)) {
      if (handle.kind !== 'directory') continue;
      const child = handle as FileSystemDirectoryHandle;
      const projectBytes = await readFileBytes(child, PROJECT_FILE);
      const sidecarBytes = await readFileBytes(child, PROJECT_FILE + AUTOSAVE_SUFFIX);
      const project = projectBytes ? await this.#client.loadProjectContainer(projectBytes) : null;
      const sidecar = sidecarBytes ? await this.#client.loadProjectContainer(sidecarBytes) : null;
      const newest = sidecar && (!project || sidecar.savedAt > project.savedAt) ? sidecar : project;
      if (!newest) continue;
      summaries.push({
        id,
        name: newest.name,
        savedAt: newest.savedAt,
        count: newest.media.length,
        hasRecovery: Boolean(sidecar && (!project || sidecar.savedAt > project.savedAt)),
        tags: newest.tags
      });
    }
    summaries.sort((a, b) => b.savedAt - a.savedAt);
    return summaries;
  }

  /**
   * Reads the home-screen index (pins and project groups). A missing file,
   * unparseable JSON, or an off-shape value all return an empty index, so the
   * caller never has to guard the file's absence or corruption.
   */
  async readHomeIndex(): Promise<HomeIndex> {
    const empty: HomeIndex = { pinned: [], groups: [] };
    let dir: FileSystemDirectoryHandle;
    try {
      dir = await projectsDir(false);
    } catch {
      return empty;
    }
    const bytes = await readFileBytes(dir, HOME_FILE);
    if (!bytes) return empty;
    try {
      return sanitizeHomeIndex(JSON.parse(new TextDecoder().decode(bytes)));
    } catch {
      return empty;
    }
  }

  /** Writes the home-screen index, creating the projects directory if needed. */
  async writeHomeIndex(index: HomeIndex): Promise<void> {
    const dir = await projectsDir(true);
    const bytes = new TextEncoder().encode(JSON.stringify(sanitizeHomeIndex(index)));
    await writeFileBytes(dir, HOME_FILE, bytes);
  }

  /** Creates an empty project directory and writes its base container. */
  async create(name: string): Promise<ProjectState> {
    const id = crypto.randomUUID();
    await projectDir(id, true);
    const project: ProjectState = {
      id,
      name: name.trim() || 'Untitled project',
      savedAt: Date.now(),
      recordings: [],
      nextMediaId: 1,
      view: null,
      description: '',
      authors: [],
      tags: [],
      groups: []
    };
    await this.writeProjectFile(project);
    return project;
  }

  /**
   * Imports audio and TextGrid files into an open project.
   *
   * Every WAV, AIFF, or FLAC file is decoded, stored under `audio/`, and
   * appended as a recording; a TextGrid whose stem matches an audio file
   * imported in the same batch attaches as its annotation. Progress is
   * reported per file so the caller can stream rows.
   *
   * @throws {Error} listing any file that is neither a recognized audio
   * container nor a TextGrid, naming each by filename; nothing in the batch
   * imports when this happens.
   */
  async importFiles(
    project: ProjectState,
    files: File[],
    onRecording?: (recording: RecordingEntry) => void
  ): Promise<void> {
    const audioFiles = files.filter((file) => isAudio(file.name));
    const textGrids = files.filter((file) => isTextGrid(file.name));
    const unsupported = files.filter((file) => !isAudio(file.name) && !isTextGrid(file.name));
    if (unsupported.length > 0) {
      const names = unsupported.map((file) => file.name).join(', ');
      const noun = unsupported.length === 1 ? 'file' : 'files';
      throw new Error(
        `${names}: unrecognized ${noun}. Phonia imports WAV, AIFF, and FLAC audio, and TextGrid annotations.`
      );
    }

    const dir = await projectDir(project.id, true);
    const audioDir = await dir.getDirectoryHandle(AUDIO_DIR, { create: true });
    const byStem = new Map<string, RecordingEntry>();

    for (const file of audioFiles) {
      const fileName = uniqueName(file.name, project.recordings);
      await writeFileStream(audioDir, fileName, file);
      const info = await this.#client.openAudioFile(
        audioSegments(project.id),
        fileName,
        fileName
      );
      const recording: RecordingEntry = {
        mediaId: project.nextMediaId++,
        name: stem(fileName),
        fileName,
        relativePath: `${AUDIO_DIR}/${fileName}`,
        hash: info.hash ?? '',
        duration: info.duration,
        sampleRate: info.sampleRate,
        channels: info.channels,
        audioId: info.id,
        annotationId: null,
        hasAnnotation: false,
        description: '',
        authors: [],
        tags: []
      };
      project.recordings.push(recording);
      project.groups.push({ Media: recording.mediaId });
      byStem.set(stem(fileName), recording);
      onRecording?.(recording);
    }

    for (const file of textGrids) {
      const recording = byStem.get(stem(file.name));
      if (!recording || recording.audioId === null) continue;
      const bytes = new Uint8Array(await file.arrayBuffer());
      const annotationId = await this.#client.importTextGrid(recording.audioId, bytes);
      recording.annotationId = annotationId;
      recording.hasAnnotation = true;
      onRecording?.(recording);
    }

    await this.writeProjectFile(project);
  }

  /**
   * Persists a finished recording as a corpus entry.
   *
   * The take already lives in the session (the engine materialized it on
   * finish), so this writes only its WAV bytes under `audio/` and appends the
   * recording — the same OPFS layout and project-file write an import produces,
   * with the content hash the engine computed. The live audio id is carried
   * straight in so the editor can open the take without decoding it again.
   */
  async addRecording(
    project: ProjectState,
    name: string,
    finished: FinishedRecordingResult
  ): Promise<RecordingEntry> {
    const dir = await projectDir(project.id, true);
    const audioDir = await dir.getDirectoryHandle(AUDIO_DIR, { create: true });
    const fileName = uniqueName(`${name}.wav`, project.recordings);
    await writeFileBytes(audioDir, fileName, finished.wav);
    const recording: RecordingEntry = {
      mediaId: project.nextMediaId++,
      name: stem(fileName),
      fileName,
      relativePath: `${AUDIO_DIR}/${fileName}`,
      hash: finished.hash,
      duration: finished.duration,
      sampleRate: finished.sampleRate,
      channels: finished.channels,
      audioId: finished.audioId,
      annotationId: null,
      hasAnnotation: false,
      description: '',
      authors: [],
      tags: []
    };
    project.recordings.push(recording);
    project.groups.push({ Media: recording.mediaId });
    await this.writeProjectFile(project);
    return recording;
  }

  /**
   * Opens a project, decoding its recordings into the session.
   *
   * When a newer autosave sidecar is present it is loaded in place of the
   * project file and reported as recovered, so unsaved work from an interrupted
   * session returns.
   */
  async open(id: string): Promise<OpenResult> {
    const dir = await projectDir(id, false);
    const projectBytes = await readFileBytes(dir, PROJECT_FILE);
    const sidecarBytes = await readFileBytes(dir, PROJECT_FILE + AUTOSAVE_SUFFIX);
    const projectMeta = projectBytes
      ? await this.#client.loadProjectContainer(projectBytes)
      : null;
    const sidecarMeta = sidecarBytes
      ? await this.#client.loadProjectContainer(sidecarBytes)
      : null;
    const recover = Boolean(
      sidecarMeta && (!projectMeta || sidecarMeta.savedAt > projectMeta.savedAt)
    );
    const container = recover ? sidecarMeta : projectMeta;
    if (!container) throw new Error('Project container is missing.');

    const audioDir = await dir.getDirectoryHandle(AUDIO_DIR, { create: true });
    const recordings: RecordingEntry[] = [];
    let nextMediaId = 1;
    for (const media of container.media) {
      const fileName = media.relativePath.split('/').pop() ?? media.relativePath;
      let audioId = null;
      let annotationId = null;
      if (await fileExists(audioDir, fileName)) {
        const info = await this.#client.openAudioFile(audioSegments(id), fileName, fileName);
        audioId = info.id;
        if (media.annotationJson) {
          annotationId = await this.#client.attachAnnotationJson(audioId, media.annotationJson);
        }
      }
      recordings.push({
        mediaId: media.mediaId,
        name: stem(fileName),
        fileName,
        relativePath: media.relativePath,
        hash: media.hash,
        duration: media.duration,
        sampleRate: media.sampleRate,
        channels: media.channels,
        audioId,
        annotationId,
        hasAnnotation: Boolean(media.annotationJson),
        description: media.description,
        authors: media.authors,
        tags: media.tags
      });
      nextMediaId = Math.max(nextMediaId, media.mediaId + 1);
    }

    const project: ProjectState = {
      id,
      name: container.name,
      savedAt: container.savedAt,
      recordings,
      nextMediaId,
      view: container.view,
      description: container.description,
      authors: container.authors,
      tags: container.tags,
      groups: container.groups.length > 0 ? container.groups : flatLibrary(recordings.map((r) => r.mediaId))
    };
    // Recovering promotes the sidecar to the project file and clears it, so the
    // recovered state becomes the saved baseline.
    if (recover) await this.writeProjectFile(project);
    return { project, recovered: recover };
  }

  /** Builds the container spec from the project's recordings. */
  #spec(project: ProjectState, savedAt: number): SaveProjectSpec {
    return {
      name: project.name,
      savedAt,
      view: project.view ?? null,
      description: project.description,
      authors: project.authors,
      tags: project.tags,
      groups: project.groups,
      media: project.recordings
        .filter((recording) => recording.audioId !== null)
        .map((recording) => ({
          mediaId: recording.mediaId,
          relativePath: recording.relativePath,
          hash: recording.hash,
          duration: recording.duration,
          sampleRate: recording.sampleRate,
          channels: recording.channels,
          annotation: recording.annotationId === null ? null : Number(recording.annotationId),
          description: recording.description,
          authors: recording.authors,
          tags: recording.tags
        }))
    };
  }

  /** Writes the project file, stamps `savedAt`, and clears any sidecar. */
  writeProjectFile(project: ProjectState): Promise<void> {
    return this.#enqueueWrite(async () => {
      // Serializing the spec inside the queued task captures the project state at
      // the moment this write runs, so the last write reflects the latest edits.
      // savedAt is kept strictly increasing so successive writes stay distinguishable.
      const now = Math.max(Date.now(), project.savedAt + 1);
      const dir = await projectDir(project.id, true);
      const bytes = await this.#client.saveProjectContainer(this.#spec(project, now));
      await writeFileBytes(dir, PROJECT_FILE, bytes);
      project.savedAt = now;
      await removeIfPresent(dir, PROJECT_FILE + AUTOSAVE_SUFFIX);
    });
  }

  /** Writes an autosave sidecar without touching the project file. */
  writeAutosave(project: ProjectState): Promise<void> {
    return this.#enqueueWrite(async () => {
      const dir = await projectDir(project.id, true);
      const bytes = await this.#client.saveProjectContainer(this.#spec(project, Date.now()));
      await writeFileBytes(dir, PROJECT_FILE + AUTOSAVE_SUFFIX, bytes);
    });
  }

  /**
   * Renames a stored project in place, whether or not it is open.
   *
   * The name lives inside the container, so the file (and a sidecar, if present)
   * is rewritten through the container rename that preserves annotations.
   */
  async rename(id: string, name: string): Promise<void> {
    const trimmed = name.trim();
    if (!trimmed) return;
    const dir = await projectDir(id, false);
    for (const file of [PROJECT_FILE, PROJECT_FILE + AUTOSAVE_SUFFIX]) {
      const bytes = await readFileBytes(dir, file);
      if (!bytes) continue;
      const renamed = await this.#client.renameProjectContainer(bytes, trimmed);
      await writeFileBytes(dir, file, renamed);
    }
  }

  /**
   * Renames an open recording: the engine's session-level name (journaled,
   * undoable) and the OPFS file it lives in, kept in lockstep since a
   * recording's display name is its file stem.
   *
   * A no-op when the trimmed name is empty or unchanged. Persists the project
   * file immediately so the new name and path survive a reload.
   */
  async renameRecording(project: ProjectState, mediaId: number, name: string): Promise<void> {
    const trimmed = name.trim();
    if (!trimmed) return;
    const entry = project.recordings.find((r) => r.mediaId === mediaId);
    if (!entry || trimmed === entry.name) return;
    const dir = await projectDir(project.id, true);
    const audioDir = await dir.getDirectoryHandle(AUDIO_DIR, { create: true });
    const ext = extension(entry.fileName) || 'wav';
    const desired = `${trimmed}.${ext}`;
    const others = project.recordings.filter((r) => r !== entry);
    const newFileName = desired === entry.fileName ? entry.fileName : uniqueName(desired, others);
    if (newFileName !== entry.fileName) {
      const bytes = await readFileBytes(audioDir, entry.fileName);
      if (bytes) {
        await writeFileBytes(audioDir, newFileName, bytes);
        await removeIfPresent(audioDir, entry.fileName);
      }
      entry.fileName = newFileName;
      entry.relativePath = `${AUDIO_DIR}/${newFileName}`;
    }
    entry.name = trimmed;
    if (entry.audioId !== null) await this.#client.renameAudio(entry.audioId, trimmed);
    await this.writeProjectFile(project);
  }

  /** Replaces a recording's description, authors, and tags, then persists. */
  async updateRecordingMetadata(
    project: ProjectState,
    mediaId: number,
    metadata: { description: string; authors: string[]; tags: string[] }
  ): Promise<void> {
    const entry = project.recordings.find((r) => r.mediaId === mediaId);
    if (!entry) return;
    entry.description = metadata.description;
    entry.authors = metadata.authors;
    entry.tags = metadata.tags;
    await this.writeProjectFile(project);
  }

  /** Replaces the project's description, authors, and tags, then persists. */
  async updateProjectMetadata(
    project: ProjectState,
    metadata: { description: string; authors: string[]; tags: string[] }
  ): Promise<void> {
    project.description = metadata.description;
    project.authors = metadata.authors;
    project.tags = metadata.tags;
    await this.writeProjectFile(project);
  }

  /** Replaces the library tree (group create/rename/dissolve/reorder), then persists. */
  async updateLibrary(project: ProjectState, groups: LibraryNode[]): Promise<void> {
    project.groups = groups;
    await this.writeProjectFile(project);
  }

  /** Persists view state (group collapse, and anything else the shell stores there). */
  async updateView(project: ProjectState, view: unknown): Promise<void> {
    project.view = view;
    await this.writeProjectFile(project);
  }

  /**
   * Permanently removes recordings previously detached from the engine
   * session, deleting their OPFS files and pruning them from the library
   * tree. The engine-side {@link WasmCoreClient.detachAudio} that preceded
   * this is journaled and undoable; calling this finalizes that removal at
   * the project level, so it belongs on the save path, after the undo window
   * for a detach has closed.
   */
  async finalizeRemovals(project: ProjectState, mediaIds: number[]): Promise<void> {
    if (mediaIds.length === 0) return;
    const remove = new Set(mediaIds);
    const dir = await projectDir(project.id, true);
    const audioDir = await dir.getDirectoryHandle(AUDIO_DIR, { create: true });
    for (const entry of project.recordings) {
      if (remove.has(entry.mediaId)) await removeIfPresent(audioDir, entry.fileName);
    }
    project.recordings = project.recordings.filter((r) => !remove.has(r.mediaId));
    project.groups = pruneMedia(project.groups, remove);
  }

  /**
   * Serializes the open project as a downloadable `.phxproj`.
   *
   * A `bundle` export embeds every recording's WAV bytes so the file restores
   * on any machine; a `references` export writes the manifest only, and an
   * importer re-links each recording by content hash against media it already
   * holds.
   */
  async exportProject(project: ProjectState, mode: ProjectExportMode): Promise<Uint8Array> {
    const spec = this.#spec(project, Math.max(Date.now(), project.savedAt));
    if (mode === 'references') return this.#client.saveProjectContainer(spec);
    const dir = await projectDir(project.id, false);
    const audioDir = await dir.getDirectoryHandle(AUDIO_DIR, { create: false });
    const media: Array<{ mediaId: number; bytes: Uint8Array }> = [];
    for (const recording of project.recordings) {
      if (recording.audioId === null) continue;
      const bytes = await readFileBytes(audioDir, recording.fileName);
      if (bytes) media.push({ mediaId: recording.mediaId, bytes });
    }
    return this.#client.saveProjectBundle(spec, media);
  }

  /**
   * Builds a self-contained `.phxproj` bundle for a stored project without
   * opening it into the app. The container's saved state is read straight from
   * OPFS; its recordings are decoded into the session only long enough to embed
   * their bytes and annotations, then detached, so a home-screen batch export
   * leaves the session as it found it. Recordings whose audio is missing are
   * omitted from the bundle.
   */
  async exportStored(id: string): Promise<{ name: string; bytes: Uint8Array }> {
    const dir = await projectDir(id, false);
    const projectBytes = await readFileBytes(dir, PROJECT_FILE);
    if (!projectBytes) throw new Error('Project container is missing.');
    const container = await this.#client.loadProjectContainer(projectBytes);
    const audioDir = await dir.getDirectoryHandle(AUDIO_DIR, { create: true });

    const opened: bigint[] = [];
    const media: Array<{ mediaId: number; bytes: Uint8Array }> = [];
    const specMedia: SaveProjectMediaSpec[] = [];
    try {
      for (const entry of container.media) {
        const fileName = entry.relativePath.split('/').pop() ?? entry.relativePath;
        const bytes = await readFileBytes(audioDir, fileName);
        if (!bytes) continue;
        const info = await this.#client.openAudioFile(audioSegments(id), fileName, fileName);
        opened.push(info.id);
        let annotation: number | null = null;
        if (entry.annotationJson) {
          const annId = await this.#client.attachAnnotationJson(info.id, entry.annotationJson);
          annotation = Number(annId);
        }
        media.push({ mediaId: entry.mediaId, bytes });
        specMedia.push({
          mediaId: entry.mediaId,
          relativePath: entry.relativePath,
          hash: entry.hash,
          duration: entry.duration,
          sampleRate: entry.sampleRate,
          channels: entry.channels,
          annotation,
          description: entry.description,
          authors: entry.authors,
          tags: entry.tags
        });
      }
      const spec: SaveProjectSpec = {
        name: container.name,
        savedAt: Math.max(Date.now(), container.savedAt),
        view: container.view ?? null,
        description: container.description,
        authors: container.authors,
        tags: container.tags,
        groups: container.groups,
        media: specMedia
      };
      const bytes = await this.#client.saveProjectBundle(spec, media);
      return { name: container.name, bytes };
    } finally {
      for (const audioId of opened) {
        try {
          await this.#client.detachAudio(audioId);
        } catch {
          // A failed cleanup only leaves an extra recording in the transient
          // session; the export bytes are already built.
        }
      }
    }
  }

  /**
   * Imports a `.phxproj` into a fresh project directory and opens it.
   *
   * A self-contained bundle restores its embedded recordings into OPFS
   * directly. A references-only file re-links each recording by content hash
   * against media already present in other projects; a recording that resolves
   * to no local file is reported as a {@link MissingMedia} gap and stays in the
   * corpus unresolved.
   */
  async importProjectFile(file: File): Promise<ImportResult> {
    const bytes = new Uint8Array(await file.arrayBuffer());
    const { meta, media } = await this.#client.readProjectBundle(bytes);
    const embedded = new Map(media.map((m) => [m.mediaId, m.bytes]));

    const id = crypto.randomUUID();
    const dir = await projectDir(id, true);
    const audioDir = await dir.getDirectoryHandle(AUDIO_DIR, { create: true });
    const gaps: MissingMedia[] = [];

    for (const entry of meta.media) {
      const fileName = entry.relativePath.split('/').pop() ?? entry.relativePath;
      const bundled = embedded.get(entry.mediaId);
      if (bundled) {
        await writeFileBytes(audioDir, fileName, bundled);
        continue;
      }
      const relinked = await this.#relinkByHash(entry.hash, id);
      if (relinked) await writeFileBytes(audioDir, fileName, relinked);
      else gaps.push({ mediaId: entry.mediaId, name: stem(fileName), hash: entry.hash });
    }

    // Persist a references-only container beside the restored media: re-saving
    // through the rename path strips any embedded bytes, so the stored project
    // stays lean and its autosave writes match.
    const container = await this.#client.renameProjectContainer(bytes, meta.name);
    await writeFileBytes(dir, PROJECT_FILE, container);

    const { project } = await this.open(id);
    return { project, gaps };
  }

  /**
   * Finds a recording's bytes by content hash among media already in OPFS,
   * skipping the project being imported into. Returns the first byte-identical
   * file found, or null when none matches.
   */
  async #relinkByHash(hash: string, skipId: string): Promise<Uint8Array | null> {
    let dir: FileSystemDirectoryHandle;
    try {
      dir = await projectsDir(false);
    } catch {
      return null;
    }
    for await (const [projectId, handle] of entries(dir)) {
      if (handle.kind !== 'directory' || projectId === skipId) continue;
      let audioDir: FileSystemDirectoryHandle;
      try {
        audioDir = await (handle as FileSystemDirectoryHandle).getDirectoryHandle(AUDIO_DIR);
      } catch {
        continue;
      }
      for await (const [name, fileHandle] of entries(audioDir)) {
        if (fileHandle.kind !== 'file') continue;
        const bytes = await readFileBytes(audioDir, name);
        if (!bytes) continue;
        if ((await this.#client.contentHash(bytes)) === hash) return bytes;
      }
    }
    return null;
  }

  /** Reads a recording's WAV bytes as a File, for playback decoding. */
  async readAudioFile(id: string, recording: RecordingEntry): Promise<File | null> {
    const dir = await projectDir(id, false);
    const audioDir = await dir.getDirectoryHandle(AUDIO_DIR, { create: false });
    const bytes = await readFileBytes(audioDir, recording.fileName);
    return bytes ? new File([bytes], recording.fileName) : null;
  }

  /** Removes a project's autosave sidecar, discarding unsaved work. */
  async discardRecovery(id: string): Promise<void> {
    const dir = await projectDir(id, false);
    await removeIfPresent(dir, PROJECT_FILE + AUTOSAVE_SUFFIX);
  }

  /** Removes a project directory and everything under it. */
  async delete(id: string): Promise<void> {
    const dir = await projectsDir(false);
    await dir.removeEntry(id, { recursive: true });
  }

  /** Copies a stored project into a fresh directory under a "copy" name. */
  async duplicate(id: string): Promise<void> {
    const source = await projectDir(id, false);
    const newId = crypto.randomUUID();
    const target = await projectDir(newId, true);
    await copyDir(source, target);
    // The byte copy carries the container's annotations intact; only the name is
    // rewritten so the grid tells original and copy apart.
    const bytes = await readFileBytes(target, PROJECT_FILE);
    if (!bytes) return;
    const meta = await this.#client.loadProjectContainer(bytes);
    const renamed = await this.#client.renameProjectContainer(bytes, `${meta.name} copy`);
    await writeFileBytes(target, PROJECT_FILE, renamed);
    await removeIfPresent(target, PROJECT_FILE + AUTOSAVE_SUFFIX);
  }
}

/** Coerces arbitrary parsed JSON into a well-formed {@link HomeIndex}. */
function sanitizeHomeIndex(value: unknown): HomeIndex {
  const source = (value ?? {}) as { pinned?: unknown; groups?: unknown };
  const strings = (list: unknown): string[] =>
    Array.isArray(list) ? list.filter((item): item is string => typeof item === 'string') : [];
  const groups: HomeProjectGroup[] = Array.isArray(source.groups)
    ? source.groups.flatMap((raw): HomeProjectGroup[] => {
        const group = raw as Partial<HomeProjectGroup>;
        if (typeof group?.id !== 'string') return [];
        return [
          {
            id: group.id,
            name: typeof group.name === 'string' ? group.name : 'Group',
            members: strings(group.members),
            collapsed: group.collapsed === true
          }
        ];
      })
    : [];
  return { pinned: strings(source.pinned), groups };
}

async function removeIfPresent(dir: FileSystemDirectoryHandle, name: string): Promise<void> {
  try {
    await dir.removeEntry(name);
  } catch {
    // Absence is not an error.
  }
}

function uniqueName(name: string, existing: RecordingEntry[]): string {
  const taken = new Set(existing.map((r) => r.fileName));
  if (!taken.has(name)) return name;
  const base = stem(name);
  const ext = extension(name);
  let i = 2;
  let candidate = `${base}-${i}.${ext}`;
  while (taken.has(candidate)) {
    i += 1;
    candidate = `${base}-${i}.${ext}`;
  }
  return candidate;
}

async function* entries(
  dir: FileSystemDirectoryHandle
): AsyncGenerator<[string, FileSystemHandle]> {
  const iterable = dir as unknown as {
    entries(): AsyncIterableIterator<[string, FileSystemHandle]>;
  };
  yield* iterable.entries();
}

async function copyDir(
  source: FileSystemDirectoryHandle,
  target: FileSystemDirectoryHandle
): Promise<void> {
  for await (const [name, handle] of entries(source)) {
    if (handle.kind === 'directory') {
      const childTarget = await target.getDirectoryHandle(name, { create: true });
      await copyDir(handle as FileSystemDirectoryHandle, childTarget);
    } else {
      const file = await (handle as FileSystemFileHandle).getFile();
      await writeFileBytes(target, name, new Uint8Array(await file.arrayBuffer()));
    }
  }
}
