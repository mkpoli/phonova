//! The in-memory project and its parts.

use crate::media::{MediaId, MediaRef};
use phx_annot::Annotation;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// A complete editing session: recordings, their annotations, named analysis
/// profiles, and an opaque view-state blob.
///
/// The project is the unit that [`crate::save`] and [`crate::load`] round-trip.
/// Loading a saved project reproduces the same recordings, per-recording
/// annotations, parameter profiles, and view state that were present when it
/// was saved.
///
/// Analysis parameters and view state are carried as [`serde_json::Value`]
/// blobs owned by the caller. The project persists and returns them unchanged;
/// it does not depend on the pitch, formant, or rendering crates that define
/// their shape.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Project {
    /// Human-facing project name.
    pub name: String,
    /// Milliseconds since the Unix epoch when this state was last written.
    ///
    /// Recovery compares this value between a project file and its autosave
    /// sidecar to decide which holds the newer state.
    pub saved_at: u64,
    /// Referenced recordings, in a stable presentation order.
    pub media: Vec<MediaRef>,
    /// Annotations keyed by the recording they belong to.
    pub annotations: BTreeMap<MediaId, Annotation>,
    /// Named parameter profiles (e.g. per speaker), each an opaque blob.
    pub profiles: Vec<Profile>,
    /// Opaque view state owned by the UI (zoom span, palette, visible tracks).
    pub view: Value,
}

impl Project {
    /// Creates an empty project with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Returns the reference for a recording, if present.
    pub fn media_ref(&self, id: MediaId) -> Option<&MediaRef> {
        self.media.iter().find(|m| m.id == id)
    }
}

/// A named set of analysis parameters, stored as an opaque blob.
///
/// The name is what the toolbar shows and switches between; `params` is the
/// caller's serialized parameter set (pitch floor/ceiling, formant ceiling,
/// palette, and so on), which this crate stores and returns without inspecting.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// Display name of the profile.
    pub name: String,
    /// Opaque serialized parameters.
    pub params: Value,
}
