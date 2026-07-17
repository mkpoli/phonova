//! The cpal-backed [`PlaybackEngine`], with its realtime stream isolated on a
//! dedicated audio thread.
//!
//! A `cpal::Stream` is `!Send` on most platforms, so it can never live in the
//! Tauri-managed state that command handlers share. This backend keeps the
//! stream on one owned thread and exposes only `Send + Sync` handles to the
//! control side: an `Arc<PlaybackClock>` (atomics) and a command channel used
//! solely to (re)build the stream when a new buffer loads. The play/pause/seek
//! fast path never touches the channel or a lock — it flips atomics the audio
//! callback reads.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, StreamConfig};

use crate::PlaybackEngine;
use crate::buffer::RenderBuffer;
use crate::clock::{PlaybackClock, render_into};
use crate::error::PlaybackError;

/// Messages the control side sends to the audio thread. The realtime controls
/// (play/pause/seek) are not here; they are atomic writes on the shared clock.
enum AudioCommand {
    /// Rebuild the output stream around a freshly loaded buffer.
    Load(Arc<RenderBuffer>),
    /// Tear the stream down and end the thread.
    Shutdown,
}

/// Native playback over a cpal output stream driven by an atomic sample clock.
pub struct CpalPlayback {
    clock: Arc<PlaybackClock>,
    tx: Sender<AudioCommand>,
    join: Option<JoinHandle<()>>,
    device_rate: u32,
    device_channels: u16,
}

impl CpalPlayback {
    /// Opens the host's default output device and starts the audio thread.
    ///
    /// The stream itself is not built until the first [`PlaybackEngine::load`];
    /// this only resolves the device and the f32 stream config the buffers will
    /// target.
    ///
    /// # Errors
    /// [`PlaybackError::NoOutputDevice`] when the host has no default output
    /// (the caller's cue to fall back to another transport), and
    /// [`PlaybackError::DeviceConfig`] / [`PlaybackError::UnsupportedConfig`]
    /// when no f32 output configuration is available.
    pub fn new() -> Result<Self, PlaybackError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(PlaybackError::NoOutputDevice)?;
        let config = choose_f32_config(&device)?;
        let device_rate = config.sample_rate;
        let device_channels = config.channels;

        let clock = Arc::new(PlaybackClock::new(0, device_rate));
        let (tx, rx) = channel();
        let thread_clock = Arc::clone(&clock);
        let join = std::thread::Builder::new()
            .name("phx-playback".to_string())
            .spawn(move || audio_thread(device, config, thread_clock, rx))
            .map_err(|err| PlaybackError::BuildStream(err.to_string()))?;

        Ok(Self {
            clock,
            tx,
            join: Some(join),
            device_rate,
            device_channels,
        })
    }

    /// The device's frame rate the buffers are rendered to.
    #[must_use]
    pub fn device_sample_rate(&self) -> u32 {
        self.device_rate
    }

    /// The device's output channel count.
    #[must_use]
    pub fn device_channels(&self) -> u16 {
        self.device_channels
    }
}

impl PlaybackEngine for CpalPlayback {
    fn load(&self, audio: &phx_audio::Audio) -> Result<(), PlaybackError> {
        let buffer = Arc::new(RenderBuffer::from_audio(
            audio,
            self.device_rate,
            self.device_channels,
        )?);
        self.clock.reset_for(buffer.frames, buffer.sample_rate);
        self.tx
            .send(AudioCommand::Load(buffer))
            .map_err(|_| PlaybackError::BuildStream("audio thread is gone".to_string()))?;
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

impl Drop for CpalPlayback {
    fn drop(&mut self) {
        let _ = self.tx.send(AudioCommand::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// The audio thread: owns the device and the current stream, rebuilding it on
/// each `Load`. The stream is played immediately and gated by the clock's
/// `playing` flag, so pause/resume never rebuild it.
fn audio_thread(
    device: Device,
    config: StreamConfig,
    clock: Arc<PlaybackClock>,
    rx: Receiver<AudioCommand>,
) {
    // The live stream, held here so it stays on this thread and nowhere else.
    let mut stream: Option<cpal::Stream> = None;
    while let Ok(command) = rx.recv() {
        match command {
            AudioCommand::Load(buffer) => {
                // Dropping the previous stream stops the old callback first.
                stream = None;
                match build_stream(&device, &config, buffer, Arc::clone(&clock)) {
                    Ok(built) => stream = Some(built),
                    Err(_) => {
                        // The device fell away between construction and load.
                        // Leave the stream absent; the poller sees no motion.
                        clock.set_playing(false);
                    }
                }
            }
            AudioCommand::Shutdown => break,
        }
    }
    drop(stream);
}

/// Builds an f32 output stream whose callback is [`render_into`], and starts it.
fn build_stream(
    device: &Device,
    config: &StreamConfig,
    buffer: Arc<RenderBuffer>,
    clock: Arc<PlaybackClock>,
) -> Result<cpal::Stream, PlaybackError> {
    let err_clock = Arc::clone(&clock);
    let stream = device
        .build_output_stream(
            *config,
            move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                render_into(&clock, &buffer, out);
            },
            move |_err| {
                // A device disconnect or xrun surfaces here; halt advancement so
                // the frontend reflects the stop rather than a frozen cursor.
                err_clock.set_playing(false);
            },
            None,
        )
        .map_err(|err| PlaybackError::BuildStream(err.to_string()))?;
    stream
        .play()
        .map_err(|err| PlaybackError::StartStream(err.to_string()))?;
    Ok(stream)
}

/// Picks an f32 output configuration: the device default when it is already
/// f32, otherwise the first supported f32 range at its maximum rate.
fn choose_f32_config(device: &Device) -> Result<StreamConfig, PlaybackError> {
    let default = device
        .default_output_config()
        .map_err(|err| PlaybackError::DeviceConfig(err.to_string()))?;
    if default.sample_format() == SampleFormat::F32 {
        return Ok(default.into());
    }
    let ranges = device
        .supported_output_configs()
        .map_err(|err| PlaybackError::DeviceConfig(err.to_string()))?;
    for range in ranges {
        if range.sample_format() == SampleFormat::F32 {
            return Ok(range.with_max_sample_rate().into());
        }
    }
    Err(PlaybackError::UnsupportedConfig(
        "device offers no f32 output configuration".to_string(),
    ))
}

/// Counts the host's output devices, so a caller can tell whether any output
/// exists (cpal 0.18's `DeviceTrait` no longer exposes device names).
///
/// # Errors
/// [`PlaybackError::DeviceConfig`] when the host cannot enumerate devices.
pub fn output_device_count() -> Result<usize, PlaybackError> {
    let host = cpal::default_host();
    let devices = host
        .output_devices()
        .map_err(|err| PlaybackError::DeviceConfig(err.to_string()))?;
    Ok(devices.count())
}
