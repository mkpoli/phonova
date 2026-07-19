//! Shared state and the data-transfer shapes the commands serialize.
//!
//! The engine lives behind a [`std::sync::Mutex`] in Tauri-managed state; every
//! command locks it for the duration of one call, so the native core sees the
//! same single-threaded command stream the web worker gives the WASM core. Ids
//! cross the IPC boundary as plain `u64` (the TypeScript client widens them back
//! to `bigint`), matching the worker protocol.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

use phx_engine::{Applied, Engine};
use phx_playback::CpalPlayback;
use serde::{Deserialize, Serialize};

/// Tauri-managed application state: the native engine, the projects root, and
/// the native playback engine.
pub struct AppState {
    /// The one journaled engine instance every command reads and mutates.
    pub engine: Mutex<Engine>,
    /// Base directory the filesystem commands resolve project paths beneath.
    pub root: PathBuf,
    /// The cpal playback engine, or `None` when the host exposes no output
    /// device. The playback commands report `None` as an error so the frontend
    /// falls back to WebAudio. Its `&self` control methods need no lock: the
    /// engine carries its own state behind atomics and an audio thread.
    pub playback: Option<CpalPlayback>,
    /// Absolute paths the OS handed this process to open (initial launch argv,
    /// a second-instance relaunch, or a macOS open-file event) that the
    /// frontend has not yet drained via `take_pending_opens`.
    pub pending_opens: Mutex<Vec<String>>,
    /// Every path ever recorded through [`AppState::pending_opens`] this
    /// session, kept after draining so `read_external_file` only ever reads a
    /// path the OS itself handed the app, never an arbitrary one a compromised
    /// webview might request.
    pub openable_paths: Mutex<HashSet<String>>,
}

impl AppState {
    /// Creates state with an empty engine rooted at `root`, opening the native
    /// output device when one is available.
    pub fn new(root: PathBuf) -> Self {
        Self {
            engine: Mutex::new(Engine::new()),
            root,
            playback: CpalPlayback::new().ok(),
            pending_opens: Mutex::new(Vec::new()),
            openable_paths: Mutex::new(HashSet::new()),
        }
    }
}

/// Duration, sample rate, channel count, name, and content hash of an import.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioInfoDto {
    pub id: u64,
    pub duration: f64,
    pub sample_rate: f64,
    pub channels: usize,
    pub name: Option<String>,
    pub hash: String,
}

/// A pitch contour as parallel arrays; `f0` holds `NaN` on unvoiced frames.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PitchTrackDto {
    pub times: Vec<f64>,
    pub f0: Vec<f64>,
    pub max_hz: f64,
}

/// Formant candidates as flat `[time, frequency, bandwidth]` triples.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormantTrackDto {
    pub points: Vec<f64>,
    pub max_hz: f64,
}

/// An intensity contour as parallel arrays of frame times and dB SPL levels.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntensityTrackDto {
    pub times: Vec<f64>,
    pub db: Vec<f64>,
}

/// One tier's identity and kind, in document order.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TierInfoDto {
    pub id: u64,
    pub name: String,
    /// `"interval"` or `"point"`.
    pub kind: &'static str,
}

/// A labeled interval bounded by two stable boundary ids.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntervalDto {
    pub id: u64,
    pub start_boundary: u64,
    pub end_boundary: u64,
    pub xmin: f64,
    pub xmax: f64,
    pub label: String,
}

/// A labeled point at a time.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PointDto {
    pub id: u64,
    pub time: f64,
    pub label: String,
}

/// A cross-document label search hit.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelHitDto {
    pub annotation: u64,
    pub tier: u64,
    /// `"interval"` or `"point"`.
    pub kind: &'static str,
    pub target: u64,
    pub start: u32,
    pub end: u32,
}

/// What a command, undo, or redo changed, for incremental UI patching.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppliedDto {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotation: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boundary: Option<u64>,
}

impl From<Applied> for AppliedDto {
    fn from(applied: Applied) -> Self {
        use phx_engine::{AnnotationId, AudioId, BoundaryId, TierId};
        let a = |id: AnnotationId| id.as_u64();
        let au = |id: AudioId| id.as_u64();
        let t = |id: TierId| id.get();
        let b = |id: BoundaryId| id.get();
        let mut dto = AppliedDto::default();
        match applied {
            Applied::AudioImported { audio } => {
                dto.kind = "audioImported".into();
                dto.audio = Some(au(audio));
            }
            Applied::AudioRemoved { audio } => {
                dto.kind = "audioRemoved".into();
                dto.audio = Some(au(audio));
            }
            Applied::AudioRenamed { audio, .. } => {
                dto.kind = "audioRenamed".into();
                dto.audio = Some(au(audio));
            }
            Applied::AudioDetached { audio, .. } => {
                dto.kind = "audioDetached".into();
                dto.audio = Some(au(audio));
            }
            Applied::AudioRestored { audio, .. } => {
                dto.kind = "audioRestored".into();
                dto.audio = Some(au(audio));
            }
            Applied::AnnotationAttached { annotation, audio } => {
                dto.kind = "annotationAttached".into();
                dto.annotation = Some(a(annotation));
                dto.audio = Some(au(audio));
            }
            Applied::AnnotationDetached { annotation } => {
                dto.kind = "annotationDetached".into();
                dto.annotation = Some(a(annotation));
            }
            Applied::TierAdded { annotation, tier } => {
                dto.kind = "tierAdded".into();
                dto.annotation = Some(a(annotation));
                dto.tier = Some(t(tier));
            }
            Applied::TierRemoved { annotation, tier } => {
                dto.kind = "tierRemoved".into();
                dto.annotation = Some(a(annotation));
                dto.tier = Some(t(tier));
            }
            Applied::TierReordered {
                annotation, tier, ..
            } => {
                dto.kind = "tierReordered".into();
                dto.annotation = Some(a(annotation));
                dto.tier = Some(t(tier));
            }
            Applied::BoundaryInserted {
                annotation,
                tier,
                boundary,
                ..
            } => {
                dto.kind = "boundaryInserted".into();
                dto.annotation = Some(a(annotation));
                dto.tier = Some(t(tier));
                dto.boundary = Some(b(boundary));
            }
            Applied::BoundaryMoved { annotation, .. } => {
                dto.kind = "boundaryMoved".into();
                dto.annotation = Some(a(annotation));
            }
            Applied::BoundaryRemoved {
                annotation,
                boundary,
            } => {
                dto.kind = "boundaryRemoved".into();
                dto.annotation = Some(a(annotation));
                dto.boundary = Some(b(boundary));
            }
            Applied::BoundaryRestored { annotation, .. } => {
                dto.kind = "boundaryRestored".into();
                dto.annotation = Some(a(annotation));
            }
            Applied::LabelSet { annotation, .. } => {
                dto.kind = "labelSet".into();
                dto.annotation = Some(a(annotation));
            }
            Applied::PointInserted {
                annotation, tier, ..
            } => {
                dto.kind = "pointInserted".into();
                dto.annotation = Some(a(annotation));
                dto.tier = Some(t(tier));
            }
            Applied::PointMoved { annotation, .. } => {
                dto.kind = "pointMoved".into();
                dto.annotation = Some(a(annotation));
            }
            Applied::PointRemoved { annotation, .. } => {
                dto.kind = "pointRemoved".into();
                dto.annotation = Some(a(annotation));
            }
        }
        dto
    }
}

/// One media entry in a [`SaveProjectSpec`], mirroring the worker's wire shape.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProjectMedia {
    pub media_id: u64,
    pub relative_path: String,
    pub hash: String,
    pub duration: f64,
    pub sample_rate: f64,
    pub channels: usize,
    #[serde(default)]
    pub annotation: Option<u64>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// The `saveProjectContainer` argument: project metadata and its media entries.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProjectSpec {
    pub name: String,
    pub saved_at: u64,
    #[serde(default)]
    pub view: serde_json::Value,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub media: Vec<SaveProjectMedia>,
    #[serde(default)]
    pub groups: Vec<phx_project::LibraryNode>,
}

/// One media entry returned by `load_project_container`, document inlined.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadProjectMedia {
    pub media_id: u64,
    pub relative_path: String,
    pub hash: String,
    pub duration: f64,
    pub sample_rate: f64,
    pub channels: usize,
    pub annotation_json: Option<String>,
    pub description: String,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
}

/// The `load_project_container` result: project metadata and its media entries.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadProjectResult {
    pub name: String,
    pub saved_at: u64,
    pub view: serde_json::Value,
    pub description: String,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub media: Vec<LoadProjectMedia>,
    pub groups: Vec<phx_project::LibraryNode>,
}

/// A figure export bundle: the main document plus any sidecar files.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBundleDto {
    pub main_name: String,
    pub main_bytes: Vec<u8>,
    pub mime: String,
    pub is_text: bool,
    pub sidecars: Vec<SidecarDto>,
}

/// A named file emitted alongside an export's main document.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarDto {
    pub name: String,
    pub bytes: Vec<u8>,
}
