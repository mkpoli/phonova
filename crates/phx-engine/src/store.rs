//! Audio store keyed by [`AudioId`].
//!
//! This is the only state an [`crate::Engine`] carries; every other engine
//! function takes an id plus explicit arguments and derives its result
//! purely from those inputs (rule 1, `docs/plan/architecture.md`).

use std::collections::HashMap;

use phx_audio::Audio;

use crate::error::EngineError;
use crate::pyramid::Pyramid;

/// Opaque handle to an audio buffer held by an [`AudioStore`].
///
/// Ids are assigned in insertion order starting from zero and are never
/// reused within a store's lifetime. The numeric value has no meaning
/// outside identifying a store entry; [`AudioId::as_u64`] and
/// [`AudioId::from_u64`] exist so a thin binding layer (`phx-wasm`) can pass
/// the id across a boundary that only understands plain numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
        assert_eq!(store.pyramid(bogus).is_err(), true);
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
