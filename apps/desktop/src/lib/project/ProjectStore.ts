import { invoke } from '@tauri-apps/api/core';
import { flatLibrary } from '@phonia/ui';
import type { LibraryNode, ProjectSummary, RecordingEntry, SaveProjectSpec } from '@phonia/ui';
import type { TauriCoreClient } from '$lib/core/TauriCoreClient';

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
  /**
   * Project-level description, authors, tags, and library tree. The desktop
   * shell does not yet expose editing for these (that UI is web-first for
   * now), but a project opened here round-trips them unchanged on save.
   */
  description: string;
  authors: string[];
  tags: string[];
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

function isWav(fileName: string): boolean {
  return extension(fileName) === 'wav';
}

/** UTF-8 → base64, for the `path` header the native `fs_write` decodes. */
function pathHeader(path: string): string {
  const bytes = new TextEncoder().encode(path);
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

/** Reads a stored file, returning `null` when it is absent. */
async function fsRead(path: string): Promise<Uint8Array<ArrayBuffer> | null> {
  try {
    const buffer = await invoke<ArrayBuffer>('fs_read', { path });
    return new Uint8Array(buffer);
  } catch {
    return null;
  }
}

async function fsWrite(path: string, bytes: Uint8Array): Promise<void> {
  await invoke('fs_write', bytes, { headers: { path: pathHeader(path) } });
}

async function fsExists(path: string): Promise<boolean> {
  return invoke<boolean>('fs_exists', { path });
}

async function fsRemove(path: string): Promise<void> {
  await invoke('fs_remove', { path });
}

async function fsListDirs(dir: string): Promise<string[]> {
  return invoke('fs_list_dirs', { dir });
}

/**
 * Orchestrates project persistence over the real filesystem, one directory per
 * project under the app data root, holding the container, its autosave sidecar,
 * and the referenced `audio/` files. The container format and its round trip
 * stay in `phx-project` (reached through the client's `saveProjectContainer` /
 * `loadProjectContainer`); this class owns the directory tree, the recovery rule
 * (a sidecar strictly newer than the project file holds unsaved work), and the
 * autosave debounce timing — the same contract the web store keeps over OPFS.
 */
export class ProjectStore {
  #client: TauriCoreClient;

  constructor(client: TauriCoreClient) {
    this.#client = client;
  }

  /** Lists every stored project, newest first, flagging pending recovery. */
  async list(): Promise<ProjectSummary[]> {
    const ids = await fsListDirs('');
    const summaries: ProjectSummary[] = [];
    for (const id of ids) {
      const projectBytes = await fsRead(`${id}/${PROJECT_FILE}`);
      const sidecarBytes = await fsRead(`${id}/${PROJECT_FILE}${AUTOSAVE_SUFFIX}`);
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
   * Every WAV is decoded, stored under `audio/`, and appended as a recording; a
   * TextGrid whose stem matches a WAV imported in the same batch attaches as its
   * annotation. Progress is reported per file so the caller can stream rows.
   */
  async importFiles(
    project: ProjectState,
    files: File[],
    onRecording?: (recording: RecordingEntry) => void
  ): Promise<void> {
    const wavs = files.filter((file) => isWav(file.name));
    const textGrids = files.filter((file) => isTextGrid(file.name));
    const byStem = new Map<string, RecordingEntry>();

    for (const file of wavs) {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const fileName = uniqueName(file.name, project.recordings);
      const relativePath = `${project.id}/${AUDIO_DIR}/${fileName}`;
      await fsWrite(relativePath, bytes);
      const info = await this.#client.openAudioStreaming(relativePath, fileName);
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
   * Opens a project, decoding its recordings into the session.
   *
   * When a newer autosave sidecar is present it is loaded in place of the
   * project file and reported as recovered, so unsaved work from an interrupted
   * session returns.
   */
  async open(id: string): Promise<OpenResult> {
    const projectBytes = await fsRead(`${id}/${PROJECT_FILE}`);
    const sidecarBytes = await fsRead(`${id}/${PROJECT_FILE}${AUTOSAVE_SUFFIX}`);
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

    const recordings: RecordingEntry[] = [];
    let nextMediaId = 1;
    for (const media of container.media) {
      const fileName = media.relativePath.split('/').pop() ?? media.relativePath;
      const relativePath = `${id}/${AUDIO_DIR}/${fileName}`;
      let audioId = null;
      let annotationId = null;
      if (await fsExists(relativePath)) {
        const info = await this.#client.openAudioStreaming(relativePath, fileName);
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
  async writeProjectFile(project: ProjectState): Promise<void> {
    const now = Date.now();
    const bytes = await this.#client.saveProjectContainer(this.#spec(project, now));
    await fsWrite(`${project.id}/${PROJECT_FILE}`, bytes);
    project.savedAt = now;
    await fsRemove(`${project.id}/${PROJECT_FILE}${AUTOSAVE_SUFFIX}`).catch(() => {});
  }

  /** Writes an autosave sidecar without touching the project file. */
  async writeAutosave(project: ProjectState): Promise<void> {
    const bytes = await this.#client.saveProjectContainer(this.#spec(project, Date.now()));
    await fsWrite(`${project.id}/${PROJECT_FILE}${AUTOSAVE_SUFFIX}`, bytes);
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
    for (const file of [PROJECT_FILE, PROJECT_FILE + AUTOSAVE_SUFFIX]) {
      const bytes = await fsRead(`${id}/${file}`);
      if (!bytes) continue;
      const renamed = await this.#client.renameProjectContainer(bytes, trimmed);
      await fsWrite(`${id}/${file}`, renamed);
    }
  }

  /** Reads a recording's WAV bytes as a File, for playback decoding. */
  async readAudioFile(id: string, recording: RecordingEntry): Promise<File | null> {
    const bytes = await fsRead(`${id}/${AUDIO_DIR}/${recording.fileName}`);
    return bytes ? new File([bytes], recording.fileName) : null;
  }

  /** Removes a project's autosave sidecar, discarding unsaved work. */
  async discardRecovery(id: string): Promise<void> {
    await fsRemove(`${id}/${PROJECT_FILE}${AUTOSAVE_SUFFIX}`).catch(() => {});
  }

  /** Removes a project directory and everything under it. */
  async delete(id: string): Promise<void> {
    await invoke('fs_remove_dir', { path: id });
  }

  /** Copies a stored project into a fresh directory under a "copy" name. */
  async duplicate(id: string): Promise<void> {
    const newId = crypto.randomUUID();
    await invoke('fs_copy_dir', { from: id, to: newId });
    const bytes = await fsRead(`${newId}/${PROJECT_FILE}`);
    if (!bytes) return;
    const meta = await this.#client.loadProjectContainer(bytes);
    const renamed = await this.#client.renameProjectContainer(bytes, `${meta.name} copy`);
    await fsWrite(`${newId}/${PROJECT_FILE}`, renamed);
    await fsRemove(`${newId}/${PROJECT_FILE}${AUTOSAVE_SUFFIX}`).catch(() => {});
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
