//! The sample-counter clock and the realtime render step.
//!
//! # Concurrency model
//!
//! Exactly one thread ever writes the playback cursor: the audio callback,
//! through [`render_into`]. Control threads (Tauri command handlers) only read
//! the cursor and write the request atomics ([`PlaybackClock::request_seek`],
//! [`PlaybackClock::set_playing`], [`PlaybackClock::set_end`]). No mutex is
//! taken on either side, and the callback never allocates: it does atomic
//! loads/stores and copies samples out of an immutable, pre-shared buffer.
//!
//! The playback position is the cursor divided by the (device) sample rate.
//! It is derived only from frames the callback has delivered to the device, so
//! it is locked to the audio stream by construction — it cannot drift the way a
//! wall-clock cursor does, whatever the device's true rate turns out to be.
//!
//! ## Atomic ordering
//!
//! * `cursor` — written `Release` by the callback after a block, read
//!   `Acquire` by `position`; the callback is the sole writer.
//! * `seek_request` — the callback consumes it with an `AcqRel` swap; a control
//!   thread publishes a target with a `Release` store. A seek posted mid-block
//!   is picked up on the next block, never torn.
//! * `playing` / `end_frame` / `total_frames` / `sample_rate` — single machine
//!   words flipped by control threads and read by the callback; `Relaxed` is
//!   enough because none of them gate access to shared memory (the sample
//!   buffer is immutable for the stream's lifetime).

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};

use crate::buffer::RenderBuffer;

/// The atomic state shared between control threads and the audio callback.
///
/// One instance backs a stream for the lifetime of a loaded buffer. It carries
/// no sample data; the samples live in the [`RenderBuffer`] the callback
/// captures. Cloning the containing `Arc` is how the callback and the control
/// side reach the same atomics.
#[derive(Debug)]
pub struct PlaybackClock {
    playing: AtomicBool,
    cursor: AtomicU64,
    seek_request: AtomicI64,
    end_frame: AtomicU64,
    total_frames: AtomicU64,
    sample_rate: AtomicU32,
}

impl PlaybackClock {
    /// Creates a stopped clock at frame zero for a `total_frames`-long buffer
    /// delivered at `sample_rate` frames per second.
    #[must_use]
    pub fn new(total_frames: u64, sample_rate: u32) -> Self {
        Self {
            playing: AtomicBool::new(false),
            cursor: AtomicU64::new(0),
            seek_request: AtomicI64::new(-1),
            end_frame: AtomicU64::new(total_frames),
            total_frames: AtomicU64::new(total_frames),
            sample_rate: AtomicU32::new(sample_rate.max(1)),
        }
    }

    /// Repoints the clock at a freshly loaded buffer, stopped at frame zero.
    pub fn reset_for(&self, total_frames: u64, sample_rate: u32) {
        self.playing.store(false, Ordering::Relaxed);
        self.total_frames.store(total_frames, Ordering::Relaxed);
        self.end_frame.store(total_frames, Ordering::Relaxed);
        self.sample_rate
            .store(sample_rate.max(1), Ordering::Relaxed);
        self.seek_request.store(-1, Ordering::Relaxed);
        self.cursor.store(0, Ordering::Release);
    }

    /// Publishes a seek to `frame`, clamped to the buffer, for the callback to
    /// adopt on its next block.
    pub fn request_seek(&self, frame: u64) {
        let total = self.total_frames.load(Ordering::Relaxed);
        let target = frame.min(total);
        // i64 cannot overflow: buffers this long (2^63 frames) are unreachable.
        self.seek_request.store(target as i64, Ordering::Release);
    }

    /// Starts or stops advancement. Stopping leaves the cursor in place.
    pub fn set_playing(&self, playing: bool) {
        self.playing.store(playing, Ordering::Relaxed);
    }

    /// Sets the frame the callback stops at; use [`u64::MAX`]-clamping via
    /// `total` for "play to the end".
    pub fn set_end(&self, end_frame: u64) {
        let total = self.total_frames.load(Ordering::Relaxed);
        self.end_frame
            .store(end_frame.min(total), Ordering::Relaxed);
    }

    /// Reports whether the callback is currently advancing the cursor.
    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }

    /// The current cursor as a frame index.
    #[must_use]
    pub fn cursor(&self) -> u64 {
        self.cursor.load(Ordering::Acquire)
    }

    /// The current playback position in seconds, derived from delivered frames.
    #[must_use]
    pub fn position_seconds(&self) -> f64 {
        let rate = f64::from(self.sample_rate.load(Ordering::Relaxed));
        self.cursor.load(Ordering::Acquire) as f64 / rate
    }

    /// The loaded buffer's duration in seconds.
    #[must_use]
    pub fn duration_seconds(&self) -> f64 {
        let rate = f64::from(self.sample_rate.load(Ordering::Relaxed));
        self.total_frames.load(Ordering::Relaxed) as f64 / rate
    }

    /// Converts a time in seconds to a frame index at the clock's rate.
    #[must_use]
    pub fn frame_at(&self, seconds: f64) -> u64 {
        if seconds <= 0.0 {
            return 0;
        }
        let rate = f64::from(self.sample_rate.load(Ordering::Relaxed));
        (seconds * rate).round() as u64
    }
}

/// Fills one interleaved output block from `buffer`, advancing `clock`.
///
/// This is the entire body of the audio callback. It is allocation-free and
/// lock-free: only atomic loads/stores and copies out of the immutable
/// `buffer`. `channels` is the device's channel count and must match
/// [`RenderBuffer::channels`]; `out.len()` must be a multiple of it.
///
/// When the cursor reaches the end frame (or the buffer end) the block is
/// filled with silence and `playing` is cleared, so a control thread polling
/// [`PlaybackClock::is_playing`] observes the stop without a callback→control
/// message.
pub fn render_into(clock: &PlaybackClock, buffer: &RenderBuffer, out: &mut [f32]) {
    let channels = buffer.channels as usize;
    if channels == 0 {
        out.fill(0.0);
        return;
    }

    let mut cursor = clock.cursor.load(Ordering::Acquire);
    let seek = clock.seek_request.swap(-1, Ordering::AcqRel);
    if seek >= 0 {
        cursor = seek as u64;
    }

    let mut playing = clock.playing.load(Ordering::Relaxed);
    let end = clock.end_frame.load(Ordering::Relaxed);
    let total = buffer.frames;
    let stop_at = end.min(total);
    let samples = buffer.samples.as_ref();

    for frame in out.chunks_mut(channels) {
        if !playing || cursor >= stop_at {
            frame.fill(0.0);
            if playing {
                // Reached the end: latch stopped so the poller sees it.
                playing = false;
            }
            continue;
        }
        let base = (cursor as usize) * channels;
        frame.copy_from_slice(&samples[base..base + channels]);
        cursor += 1;
    }

    clock.cursor.store(cursor, Ordering::Release);
    clock.playing.store(playing, Ordering::Relaxed);
}
