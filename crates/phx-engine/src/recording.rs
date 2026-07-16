//! In-progress streaming recordings.
//!
//! A capture session opens with [`RecordingStore::begin`], appends planar
//! sample chunks as they arrive from the host, and closes with
//! [`RecordingStore::finish`], which yields the accumulated planar buffer for
//! the engine to materialize into an [`crate::AudioStore`] entry. Samples
//! accumulate in memory for the take's whole duration: a ten-minute mono take
//! at 48 kHz holds about 115 MB of `f32`, which is the working bound this
//! design accepts; streaming straight to storage is a later refinement.

use std::collections::HashMap;

use crate::error::EngineError;

/// Opaque handle to a recording being captured.
///
/// Ids climb from zero and are never reused within a store's lifetime. The
/// numeric value only identifies an in-progress take; [`RecordingId::as_u64`]
/// and [`RecordingId::from_u64`] let a thin binding layer (`phx-wasm`) carry
/// the id across a boundary that speaks only plain numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RecordingId(u64);

impl RecordingId {
    /// Returns the numeric handle backing this id.
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// Reconstructs an id from a numeric handle previously returned by
    /// [`RecordingId::as_u64`].
    ///
    /// This does not check that the id names a live take; lookups with an id
    /// that was never issued, or that named a take since finished or aborted,
    /// return [`EngineError::UnknownRecordingId`].
    #[must_use]
    pub fn from_u64(value: u64) -> Self {
        Self(value)
    }
}

/// A take mid-capture: its sample rate and one growing plane per channel.
pub(crate) struct Take {
    pub(crate) sample_rate: f64,
    pub(crate) channels: Vec<Vec<f32>>,
}

/// In-memory store of takes being captured, keyed by [`RecordingId`].
#[derive(Default)]
pub(crate) struct RecordingStore {
    next_id: u64,
    takes: HashMap<RecordingId, Take>,
}

impl RecordingStore {
    /// Opens a take and returns its id.
    ///
    /// # Errors
    /// Returns [`EngineError::InvalidRequest`] when `sample_rate` is not finite
    /// and positive, or when `channels` is zero.
    pub(crate) fn begin(
        &mut self,
        sample_rate: f64,
        channels: usize,
    ) -> Result<RecordingId, EngineError> {
        if !(sample_rate.is_finite() && sample_rate > 0.0) {
            return Err(EngineError::InvalidRequest {
                reason: "recording sample_rate must be finite and positive".to_string(),
            });
        }
        if channels == 0 {
            return Err(EngineError::InvalidRequest {
                reason: "recording channels must be at least 1".to_string(),
            });
        }
        let id = RecordingId(self.next_id);
        self.next_id += 1;
        self.takes.insert(
            id,
            Take {
                sample_rate,
                channels: vec![Vec::new(); channels],
            },
        );
        Ok(id)
    }

    /// Appends one planar chunk to a take.
    ///
    /// `planar` holds the channels back to back — every channel's samples for
    /// this chunk in order, so its length must be a whole multiple of the
    /// take's channel count. Channel `c` reads from
    /// `planar[c * frames .. (c + 1) * frames]`.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownRecordingId`] when `id` names no live take,
    /// and [`EngineError::InvalidRequest`] when `planar` does not divide evenly
    /// by the channel count.
    pub(crate) fn append(&mut self, id: RecordingId, planar: &[f32]) -> Result<(), EngineError> {
        let take = self
            .takes
            .get_mut(&id)
            .ok_or(EngineError::UnknownRecordingId(id))?;
        let channels = take.channels.len();
        if !planar.len().is_multiple_of(channels) {
            return Err(EngineError::InvalidRequest {
                reason: format!(
                    "recording chunk of {} samples does not divide evenly by {channels} channels",
                    planar.len()
                ),
            });
        }
        let frames = planar.len() / channels;
        for (c, plane) in take.channels.iter_mut().enumerate() {
            plane.extend_from_slice(&planar[c * frames..(c + 1) * frames]);
        }
        Ok(())
    }

    /// Closes a take and returns its accumulated planar buffer.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownRecordingId`] when `id` names no live take.
    pub(crate) fn finish(&mut self, id: RecordingId) -> Result<Take, EngineError> {
        self.takes
            .remove(&id)
            .ok_or(EngineError::UnknownRecordingId(id))
    }

    /// Discards a take without materializing it.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownRecordingId`] when `id` names no live take.
    pub(crate) fn abort(&mut self, id: RecordingId) -> Result<(), EngineError> {
        self.takes
            .remove(&id)
            .map(|_| ())
            .ok_or(EngineError::UnknownRecordingId(id))
    }
}

/// The result of finishing a recording: the materialized store entry plus the
/// take encoded as WAV bytes for the host to persist alongside imported media.
pub struct FinishedRecording {
    /// Id of the audio buffer the take became in the store.
    pub audio: crate::AudioId,
    /// The take as a RIFF/WAVE byte buffer, ready to write to storage.
    pub wav: Vec<u8>,
}
