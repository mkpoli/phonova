//! OS integration: the file-association handoff (CLI args on Windows/Linux,
//! the macOS open-file event, and a second-instance relaunch) and the Linux
//! WebKitGTK DMABUF-renderer advisory.
//!
//! `lib.rs` records every path the OS hands the process into [`AppState`]
//! through [`record_pending_opens`]; the frontend drains it once on launch and
//! again on a `phonix://files-opened` event, then reads each file's bytes
//! through [`read_external_file`]. That command only ever serves a path the OS
//! itself handed the app — never an arbitrary one the frontend names — so a
//! compromised webview gains no general filesystem read.

use std::path::Path;

use tauri::State;

use crate::state::{AppState, DmabufAdvisoryDto};

/// Extensions the file-association handoff recognises, matched
/// case-insensitively: `.wav`, `.aiff`/`.aif`, and `.flac` recordings,
/// `.TextGrid` annotations, and the `.phxproj` project container
/// (`docs/formats/project.md`).
const RECOGNIZED_EXTENSIONS: [&str; 6] = ["wav", "aiff", "aif", "flac", "textgrid", "phxproj"];

fn recognized(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| RECOGNIZED_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

/// Filters `paths` down to existing files with a recognized extension and
/// queues them for the frontend to drain, remembering them as safe to read
/// back through [`read_external_file`].
pub fn record_pending_opens(state: &AppState, paths: impl IntoIterator<Item = String>) {
    let mut pending = state
        .pending_opens
        .lock()
        .expect("pending_opens lock poisoned");
    let mut openable = state
        .openable_paths
        .lock()
        .expect("openable_paths lock poisoned");
    for path in paths {
        if !recognized(&path) || !Path::new(&path).is_file() {
            continue;
        }
        openable.insert(path.clone());
        pending.push(path);
    }
}

/// Drains and returns the paths queued since the last drain.
#[tauri::command]
pub fn take_pending_opens(state: State<AppState>) -> Vec<String> {
    let mut pending = state
        .pending_opens
        .lock()
        .expect("pending_opens lock poisoned");
    std::mem::take(&mut pending)
}

/// Reads the bytes of a path the OS previously handed this process — one
/// already recorded via [`record_pending_opens`]. Any other path is refused.
#[tauri::command]
pub fn read_external_file(state: State<AppState>, path: String) -> Result<Vec<u8>, String> {
    let openable = state
        .openable_paths
        .lock()
        .expect("openable_paths lock poisoned");
    if !openable.contains(&path) {
        return Err("path was not handed to this app by the OS".to_string());
    }
    drop(openable);
    std::fs::read(&path).map_err(|e| e.to_string())
}

/// Reports whether the Linux WebKitGTK DMABUF-renderer advisory is relevant.
#[tauri::command]
pub fn dmabuf_advisory() -> DmabufAdvisoryDto {
    let linux = cfg!(target_os = "linux");
    let env_set = std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").as_deref() == Ok("1");
    DmabufAdvisoryDto { linux, env_set }
}
