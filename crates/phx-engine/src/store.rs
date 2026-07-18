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
use phx_spectrogram::{
    ColumnBlock, SpectrogramParams, column_block_sample_range, compute_column_block,
    compute_column_block_windowed,
};
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
        name: Option<String>,
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
            Self::Eager { audio, name, .. } => AudioInfo {
                duration: audio.duration(),
                sample_rate: audio.sample_rate(),
                channels: audio.channel_count(),
                name: name.clone(),
            },
            Self::Streamed { source, name, .. } => AudioInfo {
                duration: source.duration(),
                sample_rate: source.sample_rate(),
                channels: source.channels(),
                name: name.clone(),
            },
        }
    }

    /// Replaces the display name, returning the previous one.
    ///
    /// The name is the store's own field for both entry kinds, so a rename
    /// reaches an eager buffer and a streamed source through the same path and
    /// never depends on the decoded [`Audio`]'s embedded name.
    fn set_name(&mut self, new_name: Option<String>) -> Option<String> {
        let slot = match self {
            Self::Eager { name, .. } | Self::Streamed { name, .. } => name,
        };
        std::mem::replace(slot, new_name)
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
    /// Entries a journaled [`crate::Command::DetachAudio`] has taken out of the
    /// live set. They stay parked here — not dropped and not cloned — so undo
    /// restores a streamed source (whose pyramid is not clonable) as cheaply as
    /// an eager buffer, under the same id.
    detached: HashMap<AudioId, Entry>,
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
        let name = audio.name().map(str::to_owned);
        self.entries.insert(
            id,
            Entry::Eager {
                audio,
                pyramid,
                name,
            },
        );
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

    /// Computes one raw-dB spectrogram column block for `id` without ever
    /// holding the whole signal.
    ///
    /// An eager entry blocks straight off its resident buffer. A streamed entry
    /// decodes only the sample range the block's frames touch
    /// ([`column_block_sample_range`]) and computes the block from that window
    /// ([`compute_column_block_windowed`]), so a spectrogram tile over an
    /// hour-long take reads a few frames' worth of samples, not the whole file.
    /// Both kinds return a block bit-for-bit equal to a whole-buffer
    /// [`compute_column_block`], since the frame grid is a function of the sample
    /// rate and duration alone.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] for an unknown id and
    /// [`EngineError::Audio`] when a streamed range cannot be decoded.
    pub(crate) fn column_block(
        &self,
        id: AudioId,
        params: &SpectrogramParams,
        first_col: usize,
        col_count: usize,
    ) -> Result<ColumnBlock, EngineError> {
        match self.entries.get(&id) {
            Some(Entry::Eager { audio, .. }) => {
                let view = audio.slice_samples(0..audio.frames());
                Ok(compute_column_block(view, params, first_col, col_count))
            }
            Some(Entry::Streamed { source, .. }) => {
                let range = column_block_sample_range(
                    source.sample_rate(),
                    source.duration(),
                    params,
                    first_col,
                    col_count,
                );
                let end = range.end.min(source.frames());
                let start = range.start.min(end);
                let mono = source.read_mono(start..end)?;
                Ok(compute_column_block_windowed(
                    source.sample_rate(),
                    source.duration(),
                    params,
                    first_col,
                    col_count,
                    &mono,
                    start,
                ))
            }
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
    /// restore through [`AudioStore::restore`]. A streamed entry's undo goes
    /// through [`AudioStore::detach`]/[`AudioStore::reattach_detached`]
    /// instead — its `ByteReader` cannot be cloned to replay an import the way
    /// an eager buffer's decoded samples can — so it is never removed through
    /// this path.
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
        let name = audio.name().map(str::to_owned);
        self.entries.insert(
            id,
            Entry::Eager {
                audio,
                pyramid,
                name,
            },
        );
    }

    /// Replaces the display name of `id`, returning the previous name.
    ///
    /// Works for both eager and streamed entries; the name lives in the store,
    /// not the decoded buffer, so a streamed source renames without a decode.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] when `id` names no live entry.
    pub fn set_name(
        &mut self,
        id: AudioId,
        name: Option<String>,
    ) -> Result<Option<String>, EngineError> {
        self.entries
            .get_mut(&id)
            .map(|entry| entry.set_name(name))
            .ok_or(EngineError::UnknownAudioId(id))
    }

    /// Takes the live entry for `id` out of the store and parks it, keeping the
    /// id reserved for a later [`AudioStore::reattach_detached`].
    ///
    /// This is the forward transition of a journaled detach. It serves eager and
    /// streamed entries alike — the whole entry is moved, not dropped or cloned —
    /// so undo restores either kind by moving it back. Returns whether a live
    /// entry was parked.
    pub fn detach(&mut self, id: AudioId) -> bool {
        if let Some(entry) = self.entries.remove(&id) {
            self.detached.insert(id, entry);
            true
        } else {
            false
        }
    }

    /// Moves a parked entry back into the live set under its original id.
    ///
    /// This is the undo of [`AudioStore::detach`]. Returns whether a parked
    /// entry was restored.
    pub fn reattach_detached(&mut self, id: AudioId) -> bool {
        if let Some(entry) = self.detached.remove(&id) {
            self.entries.insert(id, entry);
            true
        } else {
            false
        }
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
