//! Audio store keyed by [`AudioId`].
//!
//! This is the only state an [`crate::Engine`] carries; every other engine
//! function takes an id plus explicit arguments and derives its result
//! purely from those inputs (rule 1, `docs/plan/architecture.md`).

use std::collections::HashMap;

use phx_audio::Audio;
use serde::{Deserialize, Serialize};

use crate::error::EngineError;
use crate::pyramid::Pyramid;

/// Opaque handle to an audio buffer held by an [`AudioStore`].
///
/// Ids are assigned in insertion order starting from zero and are never
/// reused within a store's lifetime. The numeric value has no meaning
/// outside identifying a store entry; [`AudioId::as_u64`] and
/// [`AudioId::from_u64`] exist so a thin binding layer (`phx-wasm`) can pass
/// the id across a boundary that only understands plain numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AudioId(u64);

impl AudioId {
    /// Returns the numeric handle backing this id.
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// Reconstructs an id from a numeric handle previously returned by
    /// [`AudioId::as_u64`].
    ///
    /// This does not check that the id refers to a live store entry; store
    /// lookups with an id that was never issued, or that named an entry
    /// since removed, return [`EngineError::UnknownAudioId`].
    #[must_use]
    pub fn from_u64(value: u64) -> Self {
        Self(value)
    }
}

/// Duration, sample rate, channel count, and name of a stored audio buffer.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioInfo {
    /// Duration in seconds.
    pub duration: f64,
    /// Sample rate in hertz.
    pub sample_rate: f64,
    /// Number of channels.
    pub channels: usize,
    /// Buffer name, when the import source provided one.
    pub name: Option<String>,
}

struct Entry {
    audio: Audio,
    pyramid: Pyramid,
}

/// In-memory store of imported audio buffers, keyed by [`AudioId`].
///
/// Each entry keeps its decoded [`Audio`] alongside a [`Pyramid`] built once
/// at insertion time, so repeated [`crate::Engine::waveform_slice`] calls
/// against the same id never recompute it.
#[derive(Default)]
pub struct AudioStore {
    next_id: u64,
    entries: HashMap<AudioId, Entry>,
}

impl AudioStore {
    /// Creates an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts an audio buffer, building its waveform pyramid, and returns
    /// its new id.
    pub fn insert(&mut self, audio: Audio) -> AudioId {
        let id = AudioId(self.next_id);
        self.next_id += 1;
        let pyramid = Pyramid::build(&audio);
        self.entries.insert(id, Entry { audio, pyramid });
        id
    }

    /// Returns the stored audio buffer for `id`.
    pub fn audio(&self, id: AudioId) -> Result<&Audio, EngineError> {
        self.entries
            .get(&id)
            .map(|entry| &entry.audio)
            .ok_or(EngineError::UnknownAudioId(id))
    }

    /// Returns the cached waveform pyramid for `id`.
    pub fn pyramid(&self, id: AudioId) -> Result<&Pyramid, EngineError> {
        self.entries
            .get(&id)
            .map(|entry| &entry.pyramid)
            .ok_or(EngineError::UnknownAudioId(id))
    }

    /// Reports whether `id` names a live store entry.
    #[must_use]
    pub fn contains(&self, id: AudioId) -> bool {
        self.entries.contains_key(&id)
    }

    /// Removes the entry for `id`, returning its audio buffer when present.
    ///
    /// The id is never recycled: `next_id` keeps climbing so a later
    /// [`AudioStore::insert`] can never collide with an id that undo may still
    /// restore through [`AudioStore::restore`].
    pub fn remove(&mut self, id: AudioId) -> Option<Audio> {
        self.entries.remove(&id).map(|entry| entry.audio)
    }

    /// Reinserts a previously removed buffer under its original id, rebuilding
    /// the waveform pyramid.
    ///
    /// This is the redo path for an undone import: the id is the one the
    /// original [`AudioStore::insert`] issued, so it is below `next_id` and no
    /// live entry holds it.
    pub fn restore(&mut self, id: AudioId, audio: Audio) {
        let pyramid = Pyramid::build(&audio);
        self.entries.insert(id, Entry { audio, pyramid });
    }

    /// Returns every live id in ascending order.
    ///
    /// The order is deterministic so [`crate::Engine::state_hash`] folds the
    /// store the same way regardless of insertion history.
    #[must_use]
    pub fn ids_sorted(&self) -> Vec<AudioId> {
        let mut ids: Vec<AudioId> = self.entries.keys().copied().collect();
        ids.sort_unstable();
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_audio() -> Audio {
        Audio::new(vec![vec![0.0, 0.5, -0.5, 1.0]], 8_000.0).unwrap()
    }

    #[test]
    fn ids_are_assigned_in_insertion_order_and_never_collide() {
        let mut store = AudioStore::new();
        let first = store.insert(tiny_audio());
        let second = store.insert(tiny_audio());
        assert_ne!(first, second);
        assert!(store.audio(first).is_ok());
        assert!(store.audio(second).is_ok());
    }

    #[test]
    fn unknown_id_is_a_typed_error_not_a_panic() {
        let store = AudioStore::new();
        let bogus = AudioId::from_u64(42);
        assert_eq!(store.audio(bogus), Err(EngineError::UnknownAudioId(bogus)));
        assert!(store.pyramid(bogus).is_err());
    }

    #[test]
    fn as_u64_from_u64_round_trips() {
        let mut store = AudioStore::new();
        let id = store.insert(tiny_audio());
        let round_tripped = AudioId::from_u64(id.as_u64());
        assert_eq!(id, round_tripped);
        assert!(store.audio(round_tripped).is_ok());
    }
}
