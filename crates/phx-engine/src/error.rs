//! Typed engine errors.
//!
//! Every engine entry point that takes caller-supplied data (an id, a byte
//! buffer, a time range, an analysis parameter) returns `Result<_,
//! EngineError>` instead of panicking, since those inputs come from outside
//! the process (a WASM host, eventually a native frontend) and must never be
//! trusted enough to `unwrap`.

use std::error::Error;
use std::fmt;

use phx_annot::{AnnotationError, IntegrityIssue};
use phx_audio::AudioError;

use crate::document::AnnotationId;
use crate::recording::RecordingId;
use crate::store::AudioId;

/// Errors produced by [`crate::Engine`] entry points.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineError {
    /// The given [`AudioId`] does not name a live store entry.
    UnknownAudioId(AudioId),
    /// The given [`AnnotationId`] does not name a live annotation document.
    UnknownAnnotationId(AnnotationId),
    /// The given [`RecordingId`] does not name an open capture take.
    UnknownRecordingId(RecordingId),
    /// Audio decoding failed. The wrapped [`AudioError`] distinguishes an
    /// unrecognized container from a recognized one this crate cannot decode.
    Audio(AudioError),
    /// An annotation mutation was rejected by [`phx_annot`].
    Annotation(AnnotationError),
    /// An annotation offered to [`crate::Engine::apply`] failed its own
    /// integrity check before it could enter the store.
    InvalidAnnotation(Vec<IntegrityIssue>),
    /// A tile or waveform request carried a value the underlying analysis
    /// cannot use (non-finite time/frequency bounds, a non-positive
    /// parameter, or pixel dimensions that produced no analysis columns).
    InvalidRequest {
        /// Human-readable reason.
        reason: String,
    },
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownAudioId(id) => write!(f, "unknown audio id {}", id.as_u64()),
            Self::UnknownAnnotationId(id) => write!(f, "unknown annotation id {}", id.as_u64()),
            Self::UnknownRecordingId(id) => write!(f, "unknown recording id {}", id.as_u64()),
            Self::Audio(err) => write!(f, "audio import failed: {err}"),
            Self::Annotation(err) => write!(f, "annotation mutation failed: {err}"),
            Self::InvalidAnnotation(issues) => {
                write!(
                    f,
                    "annotation failed validation with {} issue(s)",
                    issues.len()
                )
            }
            Self::InvalidRequest { reason } => write!(f, "invalid request: {reason}"),
        }
    }
}

impl Error for EngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Audio(err) => Some(err),
            Self::Annotation(err) => Some(err),
            Self::UnknownAudioId(_)
            | Self::UnknownAnnotationId(_)
            | Self::UnknownRecordingId(_)
            | Self::InvalidAnnotation(_)
            | Self::InvalidRequest { .. } => None,
        }
    }
}

impl From<AnnotationError> for EngineError {
    fn from(err: AnnotationError) -> Self {
        Self::Annotation(err)
    }
}

impl From<AudioError> for EngineError {
    fn from(err: AudioError) -> Self {
        Self::Audio(err)
    }
}
