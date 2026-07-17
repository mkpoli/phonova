//! The pre-rendered, immutable sample buffer the audio callback reads.

use std::sync::Arc;

use phx_audio::{Audio, ResampleQuality};

use crate::error::PlaybackError;

/// Interleaved `f32` samples laid out for one output device configuration.
///
/// The buffer is built once, at load time, already resampled to the device's
/// rate and widened to its channel count, so the audio callback is a straight
/// copy — no resampling, mixing, or bounds arithmetic beyond a frame index.
/// It is shared with the callback as an `Arc` and never mutated afterward.
#[derive(Debug)]
pub struct RenderBuffer {
    /// Interleaved samples, `channels` values per frame.
    pub samples: Arc<[f32]>,
    /// Channel count, matching the output device's stream config.
    pub channels: u16,
    /// Frames per second, matching the output device's stream config.
    pub sample_rate: u32,
    /// Total frames (`samples.len() / channels`).
    pub frames: u64,
}

impl RenderBuffer {
    /// Renders `audio` for a device running at `sample_rate` with `channels`
    /// output channels.
    ///
    /// Source audio is resampled to `sample_rate` and mapped onto the output
    /// channels: mono fans out to every channel, and a source with more
    /// channels wraps (`out_channel % source_channels`). An empty source yields
    /// a zero-frame buffer that plays as instant silence.
    ///
    /// # Errors
    /// Returns [`PlaybackError::Resample`] when the resampler rejects the
    /// conversion, and [`PlaybackError::UnsupportedConfig`] when `channels` is
    /// zero.
    pub fn from_audio(
        audio: &Audio,
        sample_rate: u32,
        channels: u16,
    ) -> Result<Self, PlaybackError> {
        if channels == 0 {
            return Err(PlaybackError::UnsupportedConfig(
                "output device reports zero channels".to_string(),
            ));
        }

        let resampled = audio
            .resampled(f64::from(sample_rate), ResampleQuality::Best)
            .map_err(|err| PlaybackError::Resample(err.to_string()))?;

        let src_channels = resampled.channel_count();
        let frames = resampled.frames();
        let out_channels = channels as usize;

        if src_channels == 0 || frames == 0 {
            return Ok(Self {
                samples: Arc::from(Vec::new()),
                channels,
                sample_rate,
                frames: 0,
            });
        }

        let mut interleaved = vec![0.0f32; frames * out_channels];
        for (out_ch, mapped) in (0..out_channels).map(|c| (c, c % src_channels)) {
            let source = resampled.channel(mapped);
            let mut dst = out_ch;
            for &sample in source {
                interleaved[dst] = sample;
                dst += out_channels;
            }
        }

        Ok(Self {
            samples: Arc::from(interleaved),
            channels,
            sample_rate,
            frames: frames as u64,
        })
    }

    /// Builds a buffer directly from interleaved samples, for tests and callers
    /// that already hold device-shaped audio.
    #[must_use]
    pub fn from_interleaved(samples: Arc<[f32]>, sample_rate: u32, channels: u16) -> Self {
        let frames = if channels == 0 {
            0
        } else {
            (samples.len() / channels as usize) as u64
        };
        Self {
            samples,
            channels,
            sample_rate,
            frames,
        }
    }
}
