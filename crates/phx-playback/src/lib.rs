//! Native audio playback for the Phonix desktop shell.
//!
//! Playback runs from the Rust side over a `cpal` output stream so the cursor
//! is sample-accurate and independent of the three divergent webview audio
//! stacks. The position comes from an atomic sample counter incremented in the
//! audio callback ([`clock`]), never from a wall clock, so it stays locked to
//! the audio the device actually played.
//!
//! # Shape
//!
//! [`PlaybackEngine`] is the control surface both frontends' TypeScript share
//! (`load`/`play`/`pause`/`seek`/`position`, plus range playback for
//! selections). Two implementations satisfy it:
//!
//! * `CpalPlayback` — the real backend, behind the `cpal-backend` feature. Its
//!   `!Send` stream is confined to a dedicated audio thread; the control side
//!   holds only `Send + Sync` handles.
//! * [`MockPlayback`] — a device-free backend that runs the identical render
//!   step on demand, so the trait and the clock are testable on a host with no
//!   sound device.
//!
//! # Concurrency
//!
//! The audio callback ([`clock::render_into`]) is allocation-free and
//! lock-free: it reads and writes atomics on a shared [`clock::PlaybackClock`]
//! and copies samples out of an immutable, pre-shared [`buffer::RenderBuffer`].
//! Control threads flip the same atomics; only loading a new buffer crosses a
//! channel to the audio thread. See [`clock`] for the ordering argument.

#![warn(missing_docs)]

#[cfg(feature = "cpal-backend")]
mod backend;
mod buffer;
mod clock;
mod error;
mod mock;

#[cfg(feature = "cpal-backend")]
pub use backend::{CpalPlayback, output_device_count};
pub use buffer::RenderBuffer;
pub use clock::{PlaybackClock, render_into};
pub use error::PlaybackError;
pub use mock::MockPlayback;

/// The playback control surface shared by the native and mock backends.
///
/// All methods take `&self`: the backends carry their mutable state behind
/// atomics and a channel, so a single shared handle serves every Tauri command
/// without an outer lock. The realtime controls return nothing and cannot fail
/// — they only publish an atomic the audio callback reads on its next block.
pub trait PlaybackEngine: Send + Sync {
    /// Loads `audio`, resampling it to the device configuration and rewinding
    /// to the start, stopped.
    ///
    /// # Errors
    /// Propagates [`PlaybackError`] from buffer preparation (resampling,
    /// unsupported configuration) or from handing the buffer to the stream.
    fn load(&self, audio: &phx_audio::Audio) -> Result<(), PlaybackError>;

    /// Plays from the current position to the end of the buffer.
    fn play(&self);

    /// Seeks to `seconds` and plays to the end of the buffer.
    fn play_from(&self, seconds: f64);

    /// Plays the span `[t0, t1]` (seconds), stopping at its end. Endpoints are
    /// ordered, so the arguments may arrive either way round.
    fn play_range(&self, t0: f64, t1: f64);

    /// Stops advancement, leaving the cursor where it is.
    fn pause(&self);

    /// Stops advancement and rewinds the cursor to the start.
    fn stop(&self);

    /// Moves the cursor to `seconds` without changing play/pause state.
    fn seek(&self, seconds: f64);

    /// The current position in seconds, from the atomic sample counter.
    fn position(&self) -> f64;

    /// Whether the cursor is advancing.
    fn is_playing(&self) -> bool;

    /// The loaded buffer's duration in seconds.
    fn duration(&self) -> f64;
}
