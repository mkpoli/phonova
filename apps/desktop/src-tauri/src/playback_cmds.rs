//! Native playback commands over the [`phx_playback`] engine.
//!
//! The frontend's native playback client drives these; each maps one method of
//! the shared playback interface (`load`/`play`/`pause`/`seek`, plus range
//! playback and a status poll) onto the engine held in [`AppState`]. When the
//! host has no output device the engine is absent and every command returns an
//! error, which the client reads as its cue to fall back to WebAudio.

use phx_audio::Audio;
use phx_playback::{CpalPlayback, PlaybackEngine};
use serde::Serialize;
use tauri::State;
use tauri::ipc::{InvokeBody, Request};

use crate::state::AppState;

/// The engine's current position, play state, and loaded duration, in seconds.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackStatus {
    pub position: f64,
    pub playing: bool,
    pub duration: f64,
}

/// Borrows the native engine, mapping its absence to the fall-back signal.
fn engine<'a>(state: &'a State<AppState>) -> Result<&'a CpalPlayback, String> {
    state
        .playback
        .as_ref()
        .ok_or_else(|| "native playback unavailable".to_string())
}

fn status(engine: &CpalPlayback) -> PlaybackStatus {
    PlaybackStatus {
        position: engine.position(),
        playing: engine.is_playing(),
        duration: engine.duration(),
    }
}

fn raw_body(request: &Request<'_>) -> Result<Vec<u8>, String> {
    match request.body() {
        InvokeBody::Raw(bytes) => Ok(bytes.clone()),
        InvokeBody::Json(_) => Err("expected a raw byte body".into()),
    }
}

/// Reports whether native playback is available on this host.
#[tauri::command]
pub fn playback_available(state: State<AppState>) -> bool {
    state.playback.is_some()
}

/// Decodes RIFF/WAVE bytes and loads them into the playback engine, rewound and
/// stopped. Bytes cross as a raw request body, as with `import_audio`.
#[tauri::command]
pub fn playback_load(
    state: State<AppState>,
    request: Request<'_>,
) -> Result<PlaybackStatus, String> {
    let bytes = raw_body(&request)?;
    let audio = Audio::from_wav_bytes(&bytes).map_err(|e| e.to_string())?;
    let engine = engine(&state)?;
    engine.load(&audio).map_err(|e| e.to_string())?;
    Ok(status(engine))
}

/// Seeks to `seconds` and plays to the end.
#[tauri::command]
pub fn playback_play(state: State<AppState>, seconds: f64) -> Result<PlaybackStatus, String> {
    let engine = engine(&state)?;
    engine.play_from(seconds);
    Ok(status(engine))
}

/// Plays the span `[t0, t1]`, stopping at its end.
#[tauri::command]
pub fn playback_play_range(
    state: State<AppState>,
    t0: f64,
    t1: f64,
) -> Result<PlaybackStatus, String> {
    let engine = engine(&state)?;
    engine.play_range(t0, t1);
    Ok(status(engine))
}

/// Stops advancement, holding the cursor in place.
#[tauri::command]
pub fn playback_pause(state: State<AppState>) -> Result<PlaybackStatus, String> {
    let engine = engine(&state)?;
    engine.pause();
    Ok(status(engine))
}

/// Stops advancement and rewinds to the start.
#[tauri::command]
pub fn playback_stop(state: State<AppState>) -> Result<PlaybackStatus, String> {
    let engine = engine(&state)?;
    engine.stop();
    Ok(status(engine))
}

/// Moves the cursor to `seconds` without changing play state.
#[tauri::command]
pub fn playback_seek(state: State<AppState>, seconds: f64) -> Result<PlaybackStatus, String> {
    let engine = engine(&state)?;
    engine.seek(seconds);
    Ok(status(engine))
}

/// Polls the current position and play state for the cursor.
#[tauri::command]
pub fn playback_status(state: State<AppState>) -> Result<PlaybackStatus, String> {
    Ok(status(engine(&state)?))
}
