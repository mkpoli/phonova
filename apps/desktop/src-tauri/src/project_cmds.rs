//! Native project storage: audio import into the engine, the project-container
//! round trip, and a flat filesystem surface the desktop project store drives.
//!
//! The store keeps one directory per project under the app data root — the
//! container, its autosave sidecar, and the referenced `audio/` files — exactly
//! as the web store keeps them under OPFS, so the two shells share the store
//! logic and differ only in the byte transport. Real file I/O happens here in
//! Rust; the dialog plugin only ever hands the frontend paths.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use phx_engine::{AnnotationId, ByteReader, Engine};
use phx_project::{ContentHash, FsStore, MediaId, MediaRef, Project, Storage};
use tauri::State;
use tauri::ipc::{InvokeBody, Request, Response};

use crate::state::{AppState, AudioInfoDto, LoadProjectMedia, LoadProjectResult, SaveProjectSpec};

fn store(state: &State<AppState>) -> FsStore {
    FsStore::new(state.root.clone())
}

/// Resolves a `/`-separated relative path beneath the projects root.
fn resolve(root: &Path, rel: &str) -> PathBuf {
    let mut full = root.to_path_buf();
    for part in rel.split('/').filter(|p| !p.is_empty()) {
        full.push(part);
    }
    full
}

fn raw_body(request: &Request<'_>) -> Result<Vec<u8>, String> {
    match request.body() {
        InvokeBody::Raw(bytes) => Ok(bytes.clone()),
        InvokeBody::Json(_) => Err("expected a raw byte body".into()),
    }
}

/// A `Send + Sync` file-backed [`ByteReader`] serving positioned reads without
/// holding the file in memory.
///
/// [`std::fs::File`] is `Send + Sync`, and the positioned reads used here take
/// `&self` without a seek cursor, so a streamed source can share this reader
/// across the engine's `Send + Sync` store on the native build.
struct FileReader {
    file: std::fs::File,
    len: u64,
}

impl FileReader {
    fn open(path: &Path) -> std::io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let len = file.metadata()?.len();
        Ok(Self { file, len })
    }
}

impl ByteReader for FileReader {
    fn total_len(&self) -> u64 {
        self.len
    }

    #[cfg(unix)]
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), phx_engine::AudioError> {
        use std::os::unix::fs::FileExt;
        self.file
            .read_exact_at(buf, offset)
            .map_err(|e| phx_engine::AudioError::Io(e.to_string()))
    }

    #[cfg(windows)]
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), phx_engine::AudioError> {
        use std::os::windows::fs::FileExt;
        let mut done = 0;
        while done < buf.len() {
            let read = self
                .file
                .seek_read(&mut buf[done..], offset + done as u64)
                .map_err(|e| phx_engine::AudioError::Io(e.to_string()))?;
            if read == 0 {
                return Err(phx_engine::AudioError::Io(
                    "reader ended before the requested range".to_string(),
                ));
            }
            done += read;
        }
        Ok(())
    }
}

/// Streams the file at `path` through BLAKE3 in bounded chunks, returning the
/// same content address a whole-file [`ContentHash::of`] would.
fn hash_file(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// Reports whether `rel`'s extension is `.wav`, case-insensitively.
///
/// Only WAV supports the streamed-open path below — [`phx_engine::StreamingWav`]
/// parses a RIFF/WAVE header — so an AIFF or FLAC recording always takes the
/// eager branch of [`open_audio_streaming`] regardless of length.
fn looks_like_wav(rel: &str) -> bool {
    Path::new(rel)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
}

/// Opens an audio file already stored under the projects root, choosing the
/// eager or streamed path by its length.
///
/// `rel` is a `/`-separated path beneath the root. A WAV file over the
/// engine's eager frame threshold opens streamed over a `Send + Sync` file
/// reader — only the header and the bounded waveform pyramid are read up
/// front, and analysis reads sample ranges on demand — so an hour-long take
/// never decodes whole into memory. A shorter WAV, or any AIFF or FLAC file,
/// decodes eagerly, exactly as [`import_audio`] does.
#[tauri::command]
pub fn open_audio_streaming(state: State<AppState>, rel: String) -> Result<AudioInfoDto, String> {
    let path = resolve(&state.root, &rel);
    let name = path.file_name().map(|s| s.to_string_lossy().into_owned());

    let mut engine = state
        .engine
        .lock()
        .map_err(|_| "engine lock poisoned".to_string())?;

    let (id, hash) = if looks_like_wav(&rel) {
        let frames =
            phx_engine::StreamingWav::open(FileReader::open(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?
                .frames();
        if frames > Engine::eager_import_frame_limit() {
            let hash = hash_file(&path).map_err(|e| e.to_string())?;
            let reader = FileReader::open(&path).map_err(|e| e.to_string())?;
            let id = engine
                .open_streaming_wav(reader, name)
                .map_err(|e| e.to_string())?;
            (id, hash)
        } else {
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            let hash = ContentHash::of(&bytes).to_hex();
            let id = engine
                .import_audio_bytes(&bytes)
                .map_err(|e| e.to_string())?;
            (id, hash)
        }
    } else {
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let hash = ContentHash::of(&bytes).to_hex();
        let id = engine
            .import_audio_bytes(&bytes)
            .map_err(|e| e.to_string())?;
        (id, hash)
    };

    let info = engine.audio_info(id).map_err(|e| e.to_string())?;
    Ok(AudioInfoDto {
        id: id.as_u64(),
        duration: info.duration,
        sample_rate: info.sample_rate,
        channels: info.channels,
        name: info.name,
        hash,
    })
}

/// Decodes a WAV, AIFF, or FLAC byte buffer into the engine, returning
/// metadata and the BLAKE3 content hash the project manifest records. Bytes
/// cross as a raw request body.
#[tauri::command]
pub fn import_audio(state: State<AppState>, request: Request<'_>) -> Result<AudioInfoDto, String> {
    let bytes = raw_body(&request)?;
    let hash = ContentHash::of(&bytes).to_hex();
    let mut engine = state
        .engine
        .lock()
        .map_err(|_| "engine lock poisoned".to_string())?;
    let id = engine
        .import_audio_bytes(&bytes)
        .map_err(|e| e.to_string())?;
    let info = engine.audio_info(id).map_err(|e| e.to_string())?;
    Ok(AudioInfoDto {
        id: id.as_u64(),
        duration: info.duration,
        sample_rate: info.sample_rate,
        channels: info.channels,
        name: info.name,
        hash,
    })
}

/// Serializes a project container from the spec plus the live documents it
/// names, returning the versioned ZIP bytes.
#[tauri::command]
pub fn save_project_container(
    state: State<AppState>,
    spec_json: String,
) -> Result<Vec<u8>, String> {
    let spec: SaveProjectSpec =
        serde_json::from_str(&spec_json).map_err(|e| format!("invalid project spec: {e}"))?;
    let engine = state
        .engine
        .lock()
        .map_err(|_| "engine lock poisoned".to_string())?;
    let mut project = Project::new(spec.name);
    project.saved_at = spec.saved_at;
    project.view = spec.view;
    project.description = spec.description;
    project.authors = spec.authors;
    project.tags = spec.tags;
    project.groups = spec.groups;
    for media in spec.media {
        let hash = ContentHash::from_hex(&media.hash).map_err(|e| e.to_string())?;
        let media_id = MediaId::new(media.media_id);
        project.media.push(MediaRef {
            id: media_id,
            relative_path: media.relative_path,
            hash,
            duration: media.duration,
            sample_rate: media.sample_rate,
            channels: media.channels,
            description: media.description,
            authors: media.authors,
            tags: media.tags,
        });
        if let Some(annotation_id) = media.annotation {
            let annotation = engine
                .annotation(AnnotationId::from_u64(annotation_id))
                .map_err(|e| e.to_string())?
                .clone();
            project.annotations.insert(media_id, annotation);
        }
    }
    // The host sends the tree it holds; normalize repairs it against the media
    // list that was just built rather than trusting the host to keep the two in
    // lockstep.
    project.normalize_library();
    Ok(phx_project::save(&project))
}

/// Parses a project container into the metadata the session restores from, with
/// each annotated recording's document serialized inline.
#[tauri::command]
pub fn load_project_container(bytes: Vec<u8>) -> Result<LoadProjectResult, String> {
    let project = phx_project::load(&bytes).map_err(|e| e.to_string())?;
    let mut media = Vec::with_capacity(project.media.len());
    for reference in &project.media {
        let annotation_json = match project.annotations.get(&reference.id) {
            Some(annotation) => Some(serde_json::to_string(annotation).map_err(|e| e.to_string())?),
            None => None,
        };
        media.push(LoadProjectMedia {
            media_id: reference.id.get(),
            relative_path: reference.relative_path.clone(),
            hash: reference.hash.to_hex(),
            duration: reference.duration,
            sample_rate: reference.sample_rate,
            channels: reference.channels,
            annotation_json,
            description: reference.description.clone(),
            authors: reference.authors.clone(),
            tags: reference.tags.clone(),
        });
    }
    Ok(LoadProjectResult {
        name: project.name,
        saved_at: project.saved_at,
        view: project.view,
        description: project.description,
        authors: project.authors,
        tags: project.tags,
        groups: project.groups,
        media,
    })
}

/// Rewrites a container's name, preserving media, annotations, and timestamp.
#[tauri::command]
pub fn rename_project_container(bytes: Vec<u8>, name: String) -> Result<Vec<u8>, String> {
    let mut project = phx_project::load(&bytes).map_err(|e| e.to_string())?;
    project.name = name;
    Ok(phx_project::save(&project))
}

// --- Flat filesystem surface (rooted at the projects directory) ------------

/// Reads a stored file as raw bytes.
#[tauri::command]
pub fn fs_read(state: State<AppState>, path: String) -> Result<Response, String> {
    let bytes = store(&state).read(&path).map_err(|e| e.to_string())?;
    Ok(Response::new(bytes))
}

/// Writes raw bytes to `path` (base64 in the `path` header), creating parents.
#[tauri::command]
pub fn fs_write(state: State<AppState>, request: Request<'_>) -> Result<(), String> {
    let header = request
        .headers()
        .get("path")
        .ok_or_else(|| "missing path header".to_string())?
        .to_str()
        .map_err(|e| e.to_string())?;
    let decoded = BASE64.decode(header).map_err(|e| e.to_string())?;
    let path = String::from_utf8(decoded).map_err(|e| e.to_string())?;
    let bytes = raw_body(&request)?;
    store(&state)
        .write(&path, &bytes)
        .map_err(|e| e.to_string())
}

/// Reports whether a file exists at `path`.
#[tauri::command]
pub fn fs_exists(state: State<AppState>, path: String) -> bool {
    store(&state).exists(&path)
}

/// Removes the file at `path`; absence is not an error.
#[tauri::command]
pub fn fs_remove(state: State<AppState>, path: String) -> Result<(), String> {
    store(&state).remove(&path).map_err(|e| e.to_string())
}

/// Lists the immediate file entries of `dir`.
#[tauri::command]
pub fn fs_list(state: State<AppState>, dir: String) -> Result<Vec<String>, String> {
    store(&state).list_dir(&dir).map_err(|e| e.to_string())
}

/// Removes a directory and everything under it; absence is not an error.
#[tauri::command]
pub fn fs_remove_dir(state: State<AppState>, path: String) -> Result<(), String> {
    let full = resolve(&state.root, &path);
    match std::fs::remove_dir_all(&full) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Copies a directory tree from `from` to `to`.
#[tauri::command]
pub fn fs_copy_dir(state: State<AppState>, from: String, to: String) -> Result<(), String> {
    let src = resolve(&state.root, &from);
    let dst = resolve(&state.root, &to);
    copy_dir(&src, &dst).map_err(|e| e.to_string())
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Lists the immediate subdirectory names of the projects root.
#[tauri::command]
pub fn fs_list_dirs(state: State<AppState>, dir: String) -> Result<Vec<String>, String> {
    let full = resolve(&state.root, &dir);
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&full) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.to_string()),
    };
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_dir()
            && let Some(name) = entry.file_name().to_str()
        {
            out.push(name.to_string());
        }
    }
    Ok(out)
}
