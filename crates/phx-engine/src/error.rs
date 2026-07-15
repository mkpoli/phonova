//! Typed engine errors.
//!
//! Every engine entry point that takes caller-supplied data (an id, a byte
//! buffer, a time range, an analysis parameter) returns `Result<_,
//! EngineError>` instead of panicking, since those inputs come from outside
//! the process (a WASM host, eventually a native frontend) and must never be
//! trusted enough to `unwrap`.

use std::error::Error;
use std::fmt;

use phx_audio::AudioError;

use crate::store::AudioId;

/// Errors produced by [`crate::Engine`] entry points.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineError {
    /// The given [`AudioId`] does not name a live store entry.
    UnknownAudioId(AudioId),
    /// WAV import failed.
    Audio(AudioError),
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
            Self::Audio(err) => write!(f, "audio import failed: {err}"),
            Self::InvalidRequest { reason } => write!(f, "invalid request: {reason}"),
        }
    }
}

impl Error for EngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Audio(err) => Some(err),
            Self::UnknownAudioId(_) | Self::InvalidRequest { .. } => None,
        }
    }
}

impl From<AudioError> for EngineError {
    fn from(err: AudioError) -> Self {
        Self::Audio(err)
    }
}
