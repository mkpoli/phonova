//! Project file (versioned, self-describing), media references, analysis
//! parameter profiles, autosave snapshots.
//!
//! A [`Project`] gathers a session: external recordings referenced by relative
//! path and BLAKE3 content hash, an [`phx_annot::Annotation`] per recording,
//! named analysis [`Profile`]s, and an opaque view-state blob the UI owns.
//! [`save`] serializes a project to a versioned ZIP container and [`load`]
//! reads it back to the same value.
//!
//! The container is a ZIP archive holding `manifest.json` (format tag,
//! [`FORMAT_VERSION`], name, media list), `profiles.json`, `view.json`, and one
//! `annotations/<id>.json` per annotated recording. Media stays external:
//! moving a recording is repaired by hashing candidates in sibling directories
//! ([`resolve_media`]) rather than embedding the audio.
//!
//! Every path that touches the filesystem goes through the [`Storage`] trait,
//! so the crate compiles for `wasm32` and a web host can back it with OPFS
//! while a native host uses [`FsStore`]. Autosave ([`Autosaver`]) is a debounce
//! state machine the host drives; it writes a snapshot sidecar and never blocks
//! the engine. On open, [`detect_recovery`] offers a sidecar newer than the
//! project file.
//!
//! The container format is specified in `docs/formats/project.md`.
#![warn(missing_docs)]

mod autosave;
mod container;
mod media;
mod model;
mod relink;
mod storage;

pub use autosave::{
    AUTOSAVE_SUFFIX, Autosaver, DEFAULT_DEBOUNCE_MS, DEFAULT_MAX_WAIT_MS, Recovery, autosave_path,
    detect_recovery,
};
pub use container::{FORMAT_TAG, FORMAT_VERSION, ProjectError, load, save};
pub use media::{
    ContentHash, HashParseError, MediaCandidate, MediaGap, MediaId, MediaRef, MediaResolution,
};
pub use model::{Profile, Project};
pub use relink::resolve_media;
pub use storage::{MemStore, MemStoreError, Storage, join, parent_dir};

#[cfg(not(target_arch = "wasm32"))]
pub use storage::FsStore;

#[cfg(test)]
mod tests;
