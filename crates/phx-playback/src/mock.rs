//! A headless [`PlaybackEngine`] that drives the exact realtime render step
//! without a sound device.
//!
//! On a machine with no output device (CI, WSL2) the cpal backend cannot open a
//! stream, so the trait contract and the clock are exercised here instead. The
//! mock owns a clock and a buffer and runs [`render_into`] on demand through
//! [`MockPlayback::pump`], which is what the drift integration test uses to
//! simulate a device pulling blocks. Every control method is identical to the
//! cpal backend's — they share the same clock operations — so a green mock is
//! evidence about the real control path, not a parallel reimplementation.

use std::sync::{Arc, Mutex};

use phx_audio::Audio;

use crate::PlaybackEngine;
use crate::buffer::RenderBuffer;
use crate::clock::{PlaybackClock, render_into};
use crate::error::PlaybackError;

/// A device-free playback engine for tests and headless hosts.
pub struct MockPlayback {
    clock: Arc<PlaybackClock>,
    buffer: Mutex<Arc<RenderBuffer>>,
    sample_rate: u32,
    channels: u16,
}

impl MockPlayback {
    /// Creates a mock that renders as if driven by a device at `sample_rate`
    /// with `channels` output channels.
    #[must_use]
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            clock: Arc::new(PlaybackClock::new(0, sample_rate)),
            buffer: Mutex::new(Arc::new(RenderBuffer::from_interleaved(
                Arc::from(Vec::new()),
                sample_rate,
                channels,
            ))),
            sample_rate,
            channels,
        }
    }

    /// Renders `frames` frames into a scratch buffer, exactly as a device
    /// callback would, advancing the clock. Returns the interleaved output so a
    /// test can assert on the samples as well as the position.
    ///
    /// The scratch allocation lives here, in the (non-realtime) test driver;
    /// [`render_into`] itself allocates nothing.
    pub fn pump(&self, frames: usize) -> Vec<f32> {
        let buffer = Arc::clone(&self.buffer.lock().expect("buffer lock"));
        let mut out = vec![0.0f32; frames * self.channels as usize];
        render_into(&self.clock, &buffer, &mut out);
        out
    }

    /// Borrows the shared clock, for assertions on the raw cursor.
    #[must_use]
    pub fn clock(&self) -> &Arc<PlaybackClock> {
        &self.clock
    }
}

impl PlaybackEngine for MockPlayback {
    fn load(&self, audio: &Audio) -> Result<(), PlaybackError> {
        let buffer = Arc::new(RenderBuffer::from_audio(
            audio,
            self.sample_rate,
            self.channels,
        )?);
        self.clock.reset_for(buffer.frames, buffer.sample_rate);
        *self.buffer.lock().expect("buffer lock") = buffer;
        Ok(())
    }

    fn play(&self) {
        self.clock.set_end(u64::MAX);
        self.clock.set_playing(true);
    }

    fn play_from(&self, seconds: f64) {
        self.clock.request_seek(self.clock.frame_at(seconds));
        self.clock.set_end(u64::MAX);
        self.clock.set_playing(true);
    }

    fn play_range(&self, t0: f64, t1: f64) {
        let start = self.clock.frame_at(t0.min(t1));
        let end = self.clock.frame_at(t0.max(t1));
        self.clock.request_seek(start);
        self.clock.set_end(end);
        self.clock.set_playing(true);
    }

    fn pause(&self) {
        self.clock.set_playing(false);
    }

    fn stop(&self) {
        self.clock.set_playing(false);
        self.clock.request_seek(0);
    }

    fn seek(&self, seconds: f64) {
        self.clock.request_seek(self.clock.frame_at(seconds));
    }

    fn position(&self) -> f64 {
        self.clock.position_seconds()
    }

    fn is_playing(&self) -> bool {
        self.clock.is_playing()
    }

    fn duration(&self) -> f64 {
        self.clock.duration_seconds()
    }
}
