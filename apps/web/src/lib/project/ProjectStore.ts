import type { WasmCoreClient } from '$lib/core/WasmCoreClient';
import type { ProjectSummary, RecordingEntry, SaveProjectSpec } from '$lib/core/types';

/** Directory under OPFS that holds one subdirectory per project. */
const PROJECTS_DIR = 'phonix-projects';
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

function isWav(fileName: string): boolean {
  return extension(fileName) === 'wav';
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

  constructor(client: WasmCoreClient) {
    this.#client = client;
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
        hasRecovery: Boolean(sidecar && (!project || sidecar.savedAt > project.savedAt))
      });
    }
    summaries.sort((a, b) => b.savedAt - a.savedAt);
    return summaries;
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
      view: null
    };
    await this.writeProjectFile(project);
    return project;
  }

  /**
   * Imports audio and TextGrid files into an open project.
   *
   * Every WAV is decoded, stored under `audio/`, and appended as a recording; a
   * TextGrid whose stem matches a WAV imported in the same batch attaches as its
   * annotation. Progress is reported per file so the caller can stream rows.
   */
  async importFiles(
    project: ProjectState,
    files: File[],
    onRecording?: (recording: RecordingEntry) => void
  ): Promise<void> {
    const dir = await projectDir(project.id, true);
    const audioDir = await dir.getDirectoryHandle(AUDIO_DIR, { create: true });
    const wavs = files.filter((file) => isWav(file.name));
    const textGrids = files.filter((file) => isTextGrid(file.name));
    const byStem = new Map<string, RecordingEntry>();

    for (const file of wavs) {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const fileName = uniqueName(file.name, project.recordings);
      await writeFileBytes(audioDir, fileName, bytes);
      const info = await this.#client.importAudio(new File([bytes], fileName));
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
        hasAnnotation: false
      };
      project.recordings.push(recording);
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
      const bytes = await readFileBytes(audioDir, fileName);
      let audioId = null;
      let annotationId = null;
      if (bytes) {
        const info = await this.#client.importAudio(new File([bytes], fileName));
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
        hasAnnotation: Boolean(media.annotationJson)
      });
      nextMediaId = Math.max(nextMediaId, media.mediaId + 1);
    }

    const project: ProjectState = {
      id,
      name: container.name,
      savedAt: container.savedAt,
      recordings,
      nextMediaId,
      view: container.view
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
      media: project.recordings
        .filter((recording) => recording.audioId !== null)
        .map((recording) => ({
          mediaId: recording.mediaId,
          relativePath: recording.relativePath,
          hash: recording.hash,
          duration: recording.duration,
          sampleRate: recording.sampleRate,
          channels: recording.channels,
          annotation: recording.annotationId === null ? null : Number(recording.annotationId)
        }))
    };
  }

  /** Writes the project file, stamps `savedAt`, and clears any sidecar. */
  async writeProjectFile(project: ProjectState): Promise<void> {
    const now = Date.now();
    const dir = await projectDir(project.id, true);
    const bytes = await this.#client.saveProjectContainer(this.#spec(project, now));
    await writeFileBytes(dir, PROJECT_FILE, bytes);
    project.savedAt = now;
    await removeIfPresent(dir, PROJECT_FILE + AUTOSAVE_SUFFIX);
  }

  /** Writes an autosave sidecar without touching the project file. */
  async writeAutosave(project: ProjectState): Promise<void> {
    const dir = await projectDir(project.id, true);
    const bytes = await this.#client.saveProjectContainer(this.#spec(project, Date.now()));
    await writeFileBytes(dir, PROJECT_FILE + AUTOSAVE_SUFFIX, bytes);
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
