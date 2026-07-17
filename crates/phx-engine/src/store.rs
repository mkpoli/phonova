//! Audio store keyed by [`AudioId`].
//!
//! This is the only state an [`crate::Engine`] carries; every other engine
//! function takes an id plus explicit arguments and derives its result
//! purely from those inputs (rule 1, `docs/plan/architecture.md`).
//!
//! An entry is either *eager* — the whole signal decoded into an [`Audio`] with
//! a per-sample [`Pyramid`], the path a short import or a finished recording
//! takes — or *streamed*: a [`StreamingWav`] that decodes sample ranges on
//! demand from a backing store, summarized by a bounded [`StreamPyramid`]. The
//! store hides the split behind [`AudioStore::info`], [`AudioStore::waveform`],
//! and [`AudioStore::whole`], so metadata and waveform reads never decode the
//! signal and analysis reads it only when it must.

use std::collections::HashMap;
use std::sync::Arc;

use phx_audio::{Audio, StreamingWav};
use serde::{Deserialize, Serialize};

use crate::error::EngineError;
use crate::pyramid::{MinMax, Pyramid};
use crate::stream_pyramid::StreamPyramid;

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

/// Whole-signal sample access that stays borrowed for eager buffers and owns a
/// freshly decoded buffer for streamed ones.
///
/// Analysis entry points call [`SampleAccess::audio`] and slice it; the eager
/// path pays no copy, and the streamed path materializes the signal once for
/// the call and drops it after.
pub enum SampleAccess<'a> {
    /// The store's own decoded buffer, borrowed.
    Borrowed(&'a Audio),
    /// A buffer decoded on demand from a streamed source.
    Owned(Audio),
}

impl SampleAccess<'_> {
    /// Returns the audio buffer to analyse.
    #[must_use]
    pub fn audio(&self) -> &Audio {
        match self {
            Self::Borrowed(audio) => audio,
            Self::Owned(audio) => audio,
        }
    }
}

enum Entry {
    Eager {
        audio: Audio,
        pyramid: Pyramid,
    },
    Streamed {
        source: Arc<StreamingWav>,
        pyramid: StreamPyramid,
        name: Option<String>,
    },
}

impl Entry {
    fn info(&self) -> AudioInfo {
        match self {
            Self::Eager { audio, .. } => AudioInfo {
                duration: audio.duration(),
                sample_rate: audio.sample_rate(),
                channels: audio.channel_count(),
                name: audio.name().map(str::to_owned),
            },
            Self::Streamed { source, name, .. } => AudioInfo {
                duration: source.duration(),
                sample_rate: source.sample_rate(),
                channels: source.channels(),
                name: name.clone(),
            },
        }
    }
}

/// Store of imported audio buffers, keyed by [`AudioId`].
///
/// Each eager entry keeps its decoded [`Audio`] alongside a [`Pyramid`] built
/// once at insertion; each streamed entry keeps a [`StreamingWav`] and a
/// bounded [`StreamPyramid`]. Repeated waveform reads against either never
/// recompute the pyramid.
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

    /// Inserts a fully decoded audio buffer, building its per-sample waveform
    /// pyramid, and returns its new id.
    pub fn insert(&mut self, audio: Audio) -> AudioId {
        let id = self.fresh_id();
        let pyramid = Pyramid::build(&audio);
        self.entries.insert(id, Entry::Eager { audio, pyramid });
        id
    }

    /// Inserts a streamed source and its bounded waveform pyramid, returning the
    /// new id. `name` is the display name the metadata surface reports.
    pub(crate) fn insert_streamed(
        &mut self,
        source: Arc<StreamingWav>,
        pyramid: StreamPyramid,
        name: Option<String>,
    ) -> AudioId {
        let id = self.fresh_id();
        self.entries.insert(
            id,
            Entry::Streamed {
                source,
                pyramid,
                name,
            },
        );
        id
    }

    fn fresh_id(&mut self) -> AudioId {
        let id = AudioId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Returns duration, sample rate, channel count, and name for `id` without
    /// decoding any samples.
    pub fn info(&self, id: AudioId) -> Result<AudioInfo, EngineError> {
        self.entries
            .get(&id)
            .map(Entry::info)
            .ok_or(EngineError::UnknownAudioId(id))
    }

    /// Returns whole-signal sample access for `id`: borrowed for an eager
    /// buffer, freshly decoded for a streamed one.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] for an unknown id and
    /// [`EngineError::Audio`] when a streamed source cannot be materialized.
    pub fn whole(&self, id: AudioId) -> Result<SampleAccess<'_>, EngineError> {
        match self.entries.get(&id) {
            Some(Entry::Eager { audio, .. }) => Ok(SampleAccess::Borrowed(audio)),
            Some(Entry::Streamed { source, name, .. }) => {
                let mut audio = source.materialize()?;
                if let Some(name) = name {
                    audio = audio.with_name(name.clone());
                }
                Ok(SampleAccess::Owned(audio))
            }
            None => Err(EngineError::UnknownAudioId(id)),
        }
    }

    /// Decodes an owned buffer holding exactly the frame range `[start, end)` of
    /// `id`, for viewport-window analysis (a pitch preview over a selection).
    ///
    /// The range is clamped to the frame count. The eager path copies the range
    /// and the streamed path decodes it, so both return a buffer whose sample 0
    /// is the range's first sample.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] for an unknown id and
    /// [`EngineError::Audio`] when a streamed range cannot be decoded.
    pub fn range_owned(&self, id: AudioId, start: usize, end: usize) -> Result<Audio, EngineError> {
        match self.entries.get(&id) {
            Some(Entry::Eager { audio, .. }) => {
                let frames = audio.frames();
                let start = start.min(frames);
                let end = end.min(frames).max(start);
                let planar = (0..audio.channel_count())
                    .map(|ch| audio.channel(ch)[start..end].to_vec())
                    .collect();
                Ok(Audio::new(planar, audio.sample_rate())?)
            }
            Some(Entry::Streamed { source, .. }) => {
                let planar = source.read_planar(start..end)?;
                Ok(Audio::new(planar, source.sample_rate())?)
            }
            None => Err(EngineError::UnknownAudioId(id)),
        }
    }

    /// Returns `px` min/max waveform buckets covering `[t0, t1)` seconds of `id`.
    ///
    /// Served from the entry's pyramid — the per-sample [`Pyramid`] for an eager
    /// buffer, the bounded [`StreamPyramid`] for a streamed one — so no full
    /// decode happens. Both agree bucket-for-bucket with a direct min/max scan.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] for an unknown id and
    /// [`EngineError::Audio`] when a streamed sub-bucket edge read fails.
    pub fn waveform(
        &self,
        id: AudioId,
        t0: f64,
        t1: f64,
        px: u32,
    ) -> Result<Vec<MinMax>, EngineError> {
        match self.entries.get(&id) {
            Some(Entry::Eager { pyramid, .. }) => Ok(pyramid.slice(t0, t1, px)),
            Some(Entry::Streamed {
                source, pyramid, ..
            }) => Ok(pyramid.slice(source, t0, t1, px)?),
            None => Err(EngineError::UnknownAudioId(id)),
        }
    }

    /// Returns the eager audio buffer for `id`.
    ///
    /// Only eager entries carry a resident buffer; a streamed id returns
    /// [`EngineError::InvalidRequest`]. Analysis code goes through
    /// [`AudioStore::whole`] instead, which serves both kinds.
    pub fn audio(&self, id: AudioId) -> Result<&Audio, EngineError> {
        match self.entries.get(&id) {
            Some(Entry::Eager { audio, .. }) => Ok(audio),
            Some(Entry::Streamed { .. }) => Err(EngineError::InvalidRequest {
                reason: "audio buffer is streamed; use whole() to decode it".to_string(),
            }),
            None => Err(EngineError::UnknownAudioId(id)),
        }
    }

    /// Reports whether `id` names a live store entry.
    #[must_use]
    pub fn contains(&self, id: AudioId) -> bool {
        self.entries.contains_key(&id)
    }

    /// Removes the eager entry for `id`, returning its audio buffer when present.
    ///
    /// The id is never recycled: `next_id` keeps climbing so a later
    /// [`AudioStore::insert`] can never collide with an id that undo may still
    /// restore through [`AudioStore::restore`]. Streamed entries are opened
    /// outside the journal and are not removed through this path.
    pub fn remove(&mut self, id: AudioId) -> Option<Audio> {
        match self.entries.remove(&id) {
            Some(Entry::Eager { audio, .. }) => Some(audio),
            Some(other) => {
                self.entries.insert(id, other);
                None
            }
            None => None,
        }
    }

    /// Reinserts a previously removed buffer under its original id, rebuilding
    /// the waveform pyramid.
    ///
    /// This is the redo path for an undone import: the id is the one the
    /// original [`AudioStore::insert`] issued, so it is below `next_id` and no
    /// live entry holds it.
    pub fn restore(&mut self, id: AudioId, audio: Audio) {
        let pyramid = Pyramid::build(&audio);
        self.entries.insert(id, Entry::Eager { audio, pyramid });
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
        assert_eq!(store.info(bogus), Err(EngineError::UnknownAudioId(bogus)));
        assert!(store.audio(bogus).is_err());
        assert!(store.whole(bogus).is_err());
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
